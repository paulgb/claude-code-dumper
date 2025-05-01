use clap::Parser;
use colored::*;
use db::{ClaudeDatabase, Message};
use rusqlite::Result;
use serde_json::{self, Value};

pub mod db;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Conversation ID to display
    conversation_id: Option<String>,

    /// Raw output (no JSON formatting)
    #[arg(short, long)]
    raw: bool,

    /// Custom database path (defaults to ~/.claude/__store.db)
    #[arg(short, long)]
    database_path: Option<String>,
}

fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Create database connection with custom path if provided
    let db = ClaudeDatabase::connect_with_path(args.database_path.as_deref())?;

    // If conversation_id is provided, display the conversation
    if let Some(id) = args.conversation_id {
        println!(
            "{}",
            format!("Displaying conversation with ID: {}", id)
                .cyan()
                .bold()
        );
        println!("{}", "----------------------".cyan());

        let messages = db.get_conversation(&id)?;

        if messages.is_empty() {
            println!(
                "{}",
                "No messages found for this conversation ID.".red().bold()
            );
            return Ok(());
        }

        for message in messages {
            match message {
                Message::User {
                    ref tool_use_result,
                    ..
                } => {
                    println!(
                        "{}",
                        format!("User [{}]:", format_timestamp(message.timestamp()))
                            .green()
                            .bold()
                    );

                    if args.raw {
                        println!("{}", message.message());
                    } else {
                        format_json_output(message.message());
                    }

                    if let Some(tool_result) = tool_use_result {
                        println!("\n{}", "Tool use result:".yellow().italic());

                        if args.raw {
                            println!("{}", tool_result.dimmed());
                        } else {
                            format_json_output(tool_result);
                        }
                    }
                }
                Message::Assistant {
                    ref model,
                    cost_usd,
                    duration_ms,
                    ..
                } => {
                    println!(
                        "{}",
                        format!(
                            "Assistant [{}] [{}] [${:.6}] [{}ms]:",
                            format_timestamp(message.timestamp()),
                            model,
                            cost_usd,
                            duration_ms
                        )
                        .blue()
                        .bold()
                    );

                    if args.raw {
                        println!("{}", message.message());
                    } else {
                        format_json_output(message.message());
                    }
                }
            }
            println!("{}", "----------------------".cyan());
        }

        return Ok(());
    }

    // Print conversation summaries
    let mut summaries = db.get_conversations()?;

    // Sort conversations by timestamp chronologically (oldest first)
    summaries.sort_by_key(|a| a.timestamp);

    println!("{}", "Conversations (Root to Leaf):".cyan().bold());
    println!("{}", "----------------------".cyan());
    for conversation in summaries {
        // Truncate first message to first 50 characters using the helper function
        let truncated_message = truncate_string(&conversation.first_message, 50);

        println!("{}: {}", "Leaf ID".yellow().bold(), conversation.leaf_id);
        if let Some(summary) = conversation.summary {
            println!("{}: {}", "Summary".yellow().bold(), summary);
        } else {
            println!(
                "{}: {}",
                "Summary".yellow().bold(),
                "No summary available".black()
            );
        }
        println!(
            "{}: {}",
            "Last Activity".yellow().bold(),
            format_timestamp(conversation.timestamp)
        );
        println!("{}: {}", "First Message".yellow().bold(), truncated_message);
        println!(
            "{}: {}",
            "Message Count".yellow().bold(),
            conversation.message_count
        );
        println!("{}", "----------------------".cyan());
    }

    // Add usage note after listing all conversations
    println!(
        "{}: To view a conversation, run: ./claude-viewer CONVERSATION_ID",
        "Usage".yellow().bold()
    );

    Ok(())
}

/// Format timestamp as a human-readable date and time
fn format_timestamp(timestamp: i64) -> String {
    let datetime =
        chrono::DateTime::from_timestamp(timestamp, 0).unwrap_or(chrono::DateTime::UNIX_EPOCH);
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Safely truncate a string to the specified maximum length, adding "..." if truncated
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        // Truncate at character boundary to avoid splitting UTF-8 characters
        let mut end_idx = 0;
        for (idx, _) in s.char_indices().take(max_len) {
            end_idx = idx;
        }

        // Find the next character's starting position to determine where to cut
        if let Some((next_idx, _)) = s.char_indices().nth(max_len) {
            end_idx = next_idx;
        }

        format!("{}...", &s[..end_idx])
    }
}

/// Attempt to format text as pretty JSON, or display as plain text if not valid JSON
fn format_json_output(text: &str) {
    match serde_json::from_str::<Value>(text) {
        Ok(json_value) => {
            // Pretty-print the JSON with 2-space indentation
            match serde_json::to_string_pretty(&json_value) {
                Ok(pretty) => println!("{}", pretty),
                Err(_) => println!("{}", text),
            }
        }
        Err(_) => {
            // If not valid JSON, print the original text
            println!("{}", text);
        }
    }
}
