# Claude Viewer

A command-line tool for dumping Claude API sessions, including raw tool calls and LLM requests/responses.

This is mostly intended for people who are curious in "opening up the hood" of Claude and seeing the queries it issues internally.

## Installation

Prerequisites: [Rust](https://www.rust-lang.org/tools/install), Claude Code. This has not been tested on Windows; PRs welcome if
it doesn't work.

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
```

Alternatively, you can run without installing:

```bash
# Run directly with Cargo
cargo run
cargo run CONVERSATION_ID
```

## Credits

Shamelessly vibecoded with Claude Code.
