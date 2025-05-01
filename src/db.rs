use rusqlite::{Connection, Result};
use serde_json::Value;
use std::{env, path::PathBuf};

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

    /// Gets all conversation summaries from the database with timestamp and first message
    pub fn get_conversation_summaries(&self) -> Result<Vec<(String, String, i64, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT leaf_uuid, summary FROM conversation_summaries")?;
        let summary_iter = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?, // leaf_uuid
                row.get::<_, String>(1)?, // summary text
            ))
        })?;

        let mut summaries = Vec::new();
        for summary_result in summary_iter {
            let (leaf_uuid, summary) = summary_result?;

            // Use existing get_conversation method to get all messages in chronological order
            let messages = self.get_conversation(&leaf_uuid)?;

            if !messages.is_empty() {
                // Get timestamp from the leaf message (pointed to by conversation)
                let leaf_timestamp = match &messages.last() {
                    Some(Message::User { timestamp, .. }) => *timestamp,
                    Some(Message::Assistant { timestamp, .. }) => *timestamp,
                    None => 0, // Shouldn't happen since we checked messages isn't empty
                };

                // Get first message text and parse JSON content
                let first_message = match &messages.first() {
                    Some(Message::User { message, .. }) => extract_content_from_json(message),
                    Some(Message::Assistant { message, .. }) => extract_content_from_json(message),
                    None => String::new(), // Shouldn't happen since we checked messages isn't empty
                };

                summaries.push((leaf_uuid, summary, leaf_timestamp, first_message));
            }
        }

        Ok(summaries)
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
                base_timestamp,
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
