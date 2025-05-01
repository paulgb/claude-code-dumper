use rusqlite::{Connection, Result};
use serde_json::Value;
use std::{env, path::PathBuf};
use tracing::warn;

/// Represents a conversation summary from root to leaf
pub struct Conversation {
    pub leaf_id: String,
    pub summary: Option<String>,
    pub timestamp: i64,
    pub first_message: String,
    pub message_count: usize,
}

#[derive(Clone)]
pub enum Message {
    User {
        uuid: String,
        message: String,
        tool_use_result: Option<String>,
        timestamp: i64,
        parent_uuid: Option<String>,
        session_id: String,
        cwd: String,
        user_type: String,
        version: String,
        is_sidechain: bool,
    },
    Assistant {
        uuid: String,
        message: String,
        cost_usd: f64,
        duration_ms: i64,
        is_api_error_message: bool,
        model: String,
        timestamp: i64,
        parent_uuid: Option<String>,
        session_id: String,
        cwd: String,
        user_type: String,
        version: String,
        is_sidechain: bool,
    },
}

impl Message {
    /// Get the UUID of the message
    pub fn uuid(&self) -> &str {
        match self {
            Self::User { uuid, .. } => uuid,
            Self::Assistant { uuid, .. } => uuid,
        }
    }

    /// Get the parent UUID of the message
    pub fn parent_uuid(&self) -> Option<&str> {
        match self {
            Self::User { parent_uuid, .. } => parent_uuid.as_deref(),
            Self::Assistant { parent_uuid, .. } => parent_uuid.as_deref(),
        }
    }

    /// Get the message content
    pub fn message(&self) -> &str {
        match self {
            Self::User { message, .. } => message,
            Self::Assistant { message, .. } => message,
        }
    }

    /// Get the timestamp of the message
    pub fn timestamp(&self) -> i64 {
        match self {
            Self::User { timestamp, .. } => *timestamp,
            Self::Assistant { timestamp, .. } => *timestamp,
        }
    }

    /// Get the session ID of the message
    pub fn session_id(&self) -> &str {
        match self {
            Self::User { session_id, .. } => session_id,
            Self::Assistant { session_id, .. } => session_id,
        }
    }

    /// Determine if this is a root message (has no parent)
    pub fn is_root(&self) -> bool {
        self.parent_uuid().is_none()
    }
}

pub struct ClaudeDatabase {
    conn: Connection,
}

/// Helper function to extract content from JSON message
fn extract_content_from_json(json_str: &str) -> String {
    // Try to parse as JSON and extract content field
    if let Ok(json) = serde_json::from_str::<Value>(json_str) {
        if let Some(content) = json.get("content") {
            if let Some(content_str) = content.as_str() {
                return content_str.to_string();
            }
        }
    }

    // Return original string if parsing fails or content not found
    json_str.to_string()
}

impl ClaudeDatabase {
    /// Creates a new database connection to the Claude store
    ///
    /// If `custom_path` is provided, it will be used instead of the default location.
    pub fn connect_with_path(custom_path: Option<&str>) -> Result<Self> {
        let db_path = match custom_path {
            Some(path) => PathBuf::from(path),
            None => {
                // Get user's home directory
                let home_dir = env::var("HOME").expect("Failed to get home directory");

                // Construct the default path to the SQLite database
                PathBuf::from(home_dir).join(".claude").join("__store.db")
            }
        };

        println!("Opening database at: {}", db_path.display());

        // Connect to the SQLite database
        let conn = Connection::open(db_path)?;

        Ok(Self { conn })
    }

    /// Creates a new database connection using the default path
    pub fn connect() -> Result<Self> {
        Self::connect_with_path(None)
    }

    /// Gets all root messages (messages with null parent_uuid) from the database
    pub fn get_root_messages(&self) -> Result<Vec<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT uuid FROM base_messages WHERE parent_uuid IS NULL ORDER BY timestamp ASC",
        )?;

        let root_ids = stmt.query_map([], |row| {
            row.get::<_, String>(0) // uuid
        })?;

        let mut root_messages = Vec::new();
        for id_result in root_ids {
            let id = id_result?;
            if let Ok(message) = self.get_message(&id) {
                root_messages.push(message);
            }
        }

        Ok(root_messages)
    }

    /// Gets a single message by its UUID
    fn get_message(&self, message_id: &str) -> Result<Message> {
        // Get base message info
        let mut stmt = self.conn.prepare(
            "SELECT parent_uuid, session_id, timestamp, message_type, cwd, user_type, version, isSidechain 
             FROM base_messages WHERE uuid = ?"
        )?;

        let base_row = stmt.query_row([&message_id], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?, // parent_uuid
                row.get::<_, String>(1)?,         // session_id
                row.get::<_, i64>(2)?,            // timestamp
                row.get::<_, String>(3)?,         // message_type
                row.get::<_, String>(4)?,         // cwd
                row.get::<_, String>(5)?,         // user_type
                row.get::<_, String>(6)?,         // version
                row.get::<_, i64>(7)? != 0,       // isSidechain as bool
            ))
        })?;

        let (
            parent_uuid,
            session_id,
            _base_timestamp,
            message_type,
            cwd,
            user_type,
            version,
            is_sidechain,
        ) = base_row;

        // Now get specific message details based on message_type
        if message_type == "user" {
            let mut stmt = self.conn.prepare(
                "SELECT message, tool_use_result, timestamp FROM user_messages WHERE uuid = ?",
            )?;

            let user_row = stmt.query_row([&message_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,         // message
                    row.get::<_, Option<String>>(1)?, // tool_use_result
                    row.get::<_, i64>(2)?,            // timestamp
                ))
            })?;

            let (message, tool_use_result, timestamp) = user_row;

            Ok(Message::User {
                uuid: message_id.to_string(),
                message,
                tool_use_result,
                timestamp,
                parent_uuid,
                session_id,
                cwd,
                user_type,
                version,
                is_sidechain,
            })
        } else if message_type == "assistant" {
            let mut stmt = self.conn.prepare(
                "SELECT cost_usd, duration_ms, message, is_api_error_message, timestamp, model 
                 FROM assistant_messages WHERE uuid = ?",
            )?;

            let assistant_row = stmt.query_row([&message_id], |row| {
                Ok((
                    row.get::<_, f64>(0)?,      // cost_usd
                    row.get::<_, i64>(1)?,      // duration_ms
                    row.get::<_, String>(2)?,   // message
                    row.get::<_, i64>(3)? != 0, // is_api_error_message as bool
                    row.get::<_, i64>(4)?,      // timestamp
                    row.get::<_, String>(5)?,   // model
                ))
            })?;

            let (cost_usd, duration_ms, message, is_api_error_message, timestamp, model) =
                assistant_row;

            Ok(Message::Assistant {
                uuid: message_id.to_string(),
                message,
                cost_usd,
                duration_ms,
                is_api_error_message,
                model,
                timestamp,
                parent_uuid,
                session_id,
                cwd,
                user_type,
                version,
                is_sidechain,
            })
        } else {
            Err(rusqlite::Error::QueryReturnedNoRows)
        }
    }

    /// Gets the direct child message for a given message UUID
    /// Since each message should only have one child, we log a warning if multiple children are found
    fn get_child_message(&self, parent_id: &str) -> Result<Option<Message>> {
        let mut stmt = self.conn.prepare(
            "SELECT uuid FROM base_messages WHERE parent_uuid = ? ORDER BY timestamp ASC",
        )?;

        let child_ids = stmt.query_map([parent_id], |row| {
            row.get::<_, String>(0) // uuid
        })?;

        let mut children = Vec::new();
        for id_result in child_ids {
            let id = id_result?;
            if let Ok(message) = self.get_message(&id) {
                children.push(message);
            }
        }

        // Check if we have multiple children and log a warning
        if children.len() > 1 {
            warn!(
                "Message {} has multiple children ({} found), expected only one. Using the first child.",
                parent_id,
                children.len()
            );
        }

        // Return the first child if any exist
        if !children.is_empty() {
            Ok(Some(children.remove(0)))
        } else {
            Ok(None)
        }
    }

    /// Gets all messages in a conversation thread, starting from a root message
    /// and traversing down to the leaf node following the direct child path
    pub fn get_conversation_thread(&self, root_id: &str) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut current_id = Some(root_id.to_string());

        while let Some(id) = current_id {
            if let Ok(message) = self.get_message(&id) {
                messages.push(message.clone());

                // Get the ID of the next message to process
                current_id = self
                    .get_child_message(&id)?
                    .map(|child| child.uuid().to_string());
            } else {
                break;
            }
        }

        Ok(messages)
    }

    /// Gets all conversations from the database with timestamp, first message, and message count
    /// Now uses the tree-based approach starting from root messages
    pub fn get_conversations(&self) -> Result<Vec<Conversation>> {
        // Get all root messages (messages with null parent)
        let root_messages = self.get_root_messages()?;
        let mut conversations = Vec::new();

        for root_message in root_messages {
            // Get the conversation thread from root to leaf
            let messages = self.get_conversation_thread(root_message.uuid())?;

            if messages.is_empty() {
                continue;
            }

            // Get the leaf message (last message in the thread)
            if let Some(leaf_message) = messages.last() {
                let leaf_id = leaf_message.uuid().to_string();
                let timestamp = leaf_message.timestamp();

                // Get the first message content
                let first_message = if let Some(first) = messages.first() {
                    extract_content_from_json(first.message())
                } else {
                    String::new() // Shouldn't happen
                };

                // Get the message count
                let message_count = messages.len();

                // Try to get the summary from conversation_summaries if one exists
                let summary = self.get_conversation_summary(&leaf_id)?;

                let conversation = Conversation {
                    leaf_id,
                    summary,
                    timestamp,
                    first_message,
                    message_count,
                };

                conversations.push(conversation);
            }
        }

        Ok(conversations)
    }

    /// Gets a summary for a leaf message if one exists in the conversation_summaries table
    fn get_conversation_summary(&self, leaf_id: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT summary FROM conversation_summaries WHERE leaf_uuid = ?")?;

        let result = stmt.query_row([leaf_id], |row| {
            row.get::<_, String>(0) // summary
        });

        match result {
            Ok(summary) => Ok(Some(summary)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Gets a complete conversation starting from a leaf message and following all parents
    pub fn get_conversation(&self, leaf: &str) -> Result<Vec<Message>> {
        let mut messages = Vec::new();
        let mut current_id = Some(leaf.to_string());

        while let Some(id) = current_id {
            // Get base message info
            let mut stmt = self.conn.prepare(
                "SELECT parent_uuid, session_id, timestamp, message_type, cwd, user_type, version, isSidechain 
                 FROM base_messages WHERE uuid = ?"
            )?;

            let base_row = stmt.query_row([&id], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?, // parent_uuid
                    row.get::<_, String>(1)?,         // session_id
                    row.get::<_, i64>(2)?,            // timestamp
                    row.get::<_, String>(3)?,         // message_type
                    row.get::<_, String>(4)?,         // cwd
                    row.get::<_, String>(5)?,         // user_type
                    row.get::<_, String>(6)?,         // version
                    row.get::<_, i64>(7)? != 0,       // isSidechain as bool
                ))
            })?;

            let (
                parent_uuid,
                session_id,
                _base_timestamp,
                message_type,
                cwd,
                user_type,
                version,
                is_sidechain,
            ) = base_row;

            // Now get specific message details based on message_type
            if message_type == "user" {
                let mut stmt = self.conn.prepare(
                    "SELECT message, tool_use_result, timestamp FROM user_messages WHERE uuid = ?",
                )?;

                let user_row = stmt.query_row([&id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,         // message
                        row.get::<_, Option<String>>(1)?, // tool_use_result
                        row.get::<_, i64>(2)?,            // timestamp
                    ))
                })?;

                let (message, tool_use_result, timestamp) = user_row;

                messages.push(Message::User {
                    uuid: id.clone(),
                    message,
                    tool_use_result,
                    timestamp,
                    parent_uuid: parent_uuid.clone(),
                    session_id,
                    cwd,
                    user_type,
                    version,
                    is_sidechain,
                });
            } else if message_type == "assistant" {
                let mut stmt = self.conn.prepare(
                    "SELECT cost_usd, duration_ms, message, is_api_error_message, timestamp, model 
                     FROM assistant_messages WHERE uuid = ?",
                )?;

                let assistant_row = stmt.query_row([&id], |row| {
                    Ok((
                        row.get::<_, f64>(0)?,      // cost_usd
                        row.get::<_, i64>(1)?,      // duration_ms
                        row.get::<_, String>(2)?,   // message
                        row.get::<_, i64>(3)? != 0, // is_api_error_message as bool
                        row.get::<_, i64>(4)?,      // timestamp
                        row.get::<_, String>(5)?,   // model
                    ))
                })?;

                let (cost_usd, duration_ms, message, is_api_error_message, timestamp, model) =
                    assistant_row;

                messages.push(Message::Assistant {
                    uuid: id.clone(),
                    message,
                    cost_usd,
                    duration_ms,
                    is_api_error_message,
                    model,
                    timestamp,
                    parent_uuid: parent_uuid.clone(),
                    session_id,
                    cwd,
                    user_type,
                    version,
                    is_sidechain,
                });
            }

            // Move to parent message
            current_id = parent_uuid;
        }

        // Reverse to get messages in chronological order
        messages.reverse();

        Ok(messages)
    }
}
