# Claude Viewer

A command-line tool for viewing Claude API conversation history.

## Installation

You can install Claude Viewer using Cargo:

```bash
# Install the binary to your PATH
cargo install --path .
```

## Usage

After installation, you can use the `claude-viewer` command to view your conversation history:

```bash
# List all conversations
claude-viewer

# View a specific conversation by ID
claude-viewer CONVERSATION_ID

# View raw output (no JSON formatting)
claude-viewer CONVERSATION_ID --raw
```

Alternatively, you can run without installing:

```bash
# Run directly with Cargo
cargo run
cargo run CONVERSATION_ID
cargo run CONVERSATION_ID -- --raw
```

## Features

- View a list of all saved conversations with summaries
- Display detailed conversation history for a specific ID
- View raw message content or formatted JSON
- Displays timing, model, and cost information for each request