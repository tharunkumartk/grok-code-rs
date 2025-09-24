# Grok Code
A terminal-based coding assistant powered by AI. Grok Code provides an interactive TUI (Text User Interface) for chatting with an AI agent that can analyze codebases, execute tools for file operations, search, and run shell commands. Built in Rust for performance and safety.

## Features
- **Interactive TUI**: Chat with the AI in a full-screen terminal interface with markdown support, scrolling, and multi-panel layout (chat, tools, input).
- **AI Agent Integration**: Uses OpenRouter API (default model: xAI Grok) for intelligent responses with tool calling capabilities.
- **Tool Support**:
  - File reading (`fs.read`) with optional byte ranges and encoding.
  - Codebase searching (`fs.search`) with regex, glob patterns, and case sensitivity.
  - File writing (`fs.write`) with create/overwrite options.
  - Patch application (`fs.apply_patch`) with dry-run support.
  - File finding (`fs.find`) with fuzzy matching and file type filtering.
  - Code symbol extraction (`code.symbols`) for functions, classes, structs, and more across multiple languages.
  - Shell command execution (`shell.exec`) with timeouts, environment vars, and streaming output.
- **Safety Features**: Tool outputs are truncated to prevent token limits; approval requests for destructive operations (simplified in current version).
- **Event-Driven Architecture**: Asynchronous event bus for handling agent responses, tool progress, and UI updates.
- **Multi-Crate Structure**: Modular design with `core` (logic), `tui` (interface), and `cli` (entry point).

## Quick Start

### Prerequisites
- Rust (1.75+) and Cargo.
- OpenRouter API key (free tier available).

### Setup
1. Clone the repository:
   ```
   git clone <repo-url>
   cd grok-code
   ```

2. Copy the environment file and add your API key:
   ```
   cp .env.example .env
   # Edit .env and set OPENROUTER_API_KEY=your-key-here
   ```
   Get a free key from [OpenRouter](https://openrouter.ai/keys).

3. Build and run:
   ```
   cargo run
   ```
   This launches the TUI via the `cli` binary.

### Usage
- **Run the App**: `cargo run` starts the interactive TUI.
- **Chat**: Type messages in the input area (bottom panel) and press Enter. The AI responds in the chat panel.
- **Navigation**:
  - `Tab`: Switch focus between input, chat, and tools panels.
  - `↑`/`↓` or scroll wheel: Scroll in the focused panel.
  - `End`: Jump to bottom of chat or tools (re-enables auto-scroll).
  - `q` or `Ctrl+C`: Quit.
- **Commands**:
  - `/clear`: Clear conversation history.
  - `/info` or `/q`: Show agent info or quit.
  - `/context`: Display current token usage statistics.
- **Tools in Action**: The agent automatically uses tools (e.g., "read src/main.rs" to view a file). Tool output appears in the tools panel with real-time streaming (stdout/stderr).
- **Markdown Support**: Agent responses render with bold, italics, code blocks, lists, and quotes.

Example interaction:
- User: "What's in the main file?"
- Agent: Uses `fs.read` tool, displays file contents, and explains.

## Project Structure
```
.
├── Cargo.toml          # Workspace config
├── core/               # Core logic
│   ├── src/agent/      # AI agent (OpenRouter implementation)
│   ├── src/session.rs  # Conversation/session management
│   ├── src/tools/      # Tool definitions and executor (fs ops, shell)
│   └── src/events.rs   # Event bus for async communication
├── tui/                # Terminal UI
│   ├── src/main.rs     # TUI entry point
│   ├── src/app.rs      # Main app state and rendering
│   └── src/markdown.rs # Markdown-to-TUI rendering
└── cli/                # CLI wrapper
    └── src/main.rs     # Launches TUI
```

- **Dependencies**: Tokio (async), Ratatui/Crossterm (TUI), Reqwest (HTTP), Serde (JSON), Pulldown-cmark (Markdown).
- **Workspace**: Shared deps in root `Cargo.toml` for consistency.

## Architecture Overview
1. **Event Bus**: Central async channel (`tokio::sync::mpsc`) for events like `AppEvent::AgentResponse`, `ToolBegin`, etc.
2. **Session**: Manages chat history (`ChatMessage`), active tools (`ActiveTool`), and interacts with the agent.
3. **Agent**: `MultiModelAgent` handles LLM calls with tool calling (OpenAI-compatible format). Supports up to 8 tool turns with automatic fallback between model providers.
4. **Tools**: `ToolExecutor` implements real operations (e.g., `tokio::fs` for files, `tokio::process` for shell). Results are JSON-structured.
5. **TUI**: Ratatui-based with panels for chat (markdown-rendered), tools (progress/output), and input. Handles keyboard/mouse events.
6. **Safety**: Validates tool args, truncates large outputs (default 1MB), timeouts (e.g., 30s for shell).

The app runs in a `tokio::main` loop, processing terminal events and app events concurrently.

## Customization
- **Model**: Set `OPENROUTER_MODEL` in `.env` (default: `x-ai/grok-4-fast:free`).
- **Max Tool Output**: `GROK_TOOL_MAX_OUTPUT_SIZE` env var (bytes).
- **Extend Tools**: Add new `ToolName` variants and handlers in `core/src/tools/executor.rs`.
- **New Agent**: Implement `Agent` trait in `core/src/agent/` and use `AgentFactory`.

## Development
- **Build**: `cargo build` (or `cargo build --release` for optimized).
- **Run Tests**: `cargo test` (core event bus and basic tools).
- **Tracing**: Logs at WARN level by default; set `RUST_LOG=debug` for more.
- **Hot Reload**: Use `cargo watch -x run` for dev.

## Limitations & Roadmap
- **Patch Tool**: Basic unified diff parser; needs robust hunk application.
- **Approval UI**: Auto-approves for demo; add interactive prompts.
- **Streaming Responses**: Deltas accumulate; implement true streaming.
- **Headless Mode**: CLI currently launches TUI; add non-interactive mode.
- **More Agents**: Support local models (e.g., via Ollama).
- **Error Handling**: Expand for edge cases like invalid paths or regex.

## Contributing
Fork, branch, and submit PRs! Focus on safety, performance, and UX. See issues for ideas.

## License
MIT License - see [LICENSE](LICENSE) (add if missing).

---

Built with ❤️ using Rust. Questions? Open an issue!