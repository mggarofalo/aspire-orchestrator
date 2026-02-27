# AspireOrchestrator (Rust)

Manages isolated .NET Aspire development slots via tmux sessions, enabling 4-6 concurrent Claude Code agents on separate features.

## Architecture

- **Workspace**: `ao-core` (library) + `ao-tui` (ratatui binary)
- **Hybrid process management**: Direct `tokio::process` for Aspire stacks, tmux for Claude Code sessions
- **Async runtime**: tokio with `RwLock` for shared state

## Toolchain

Uses `stable-x86_64-pc-windows-gnu` (set as rustup default). Requires MinGW-w64 on PATH:
- MSYS2 installed at `C:\msys64`
- `C:\msys64\mingw64\bin` must be in PATH (provides `gcc`, `dlltool`, `ld`)

## Build & Test

```bash
cargo build                 # build all
cargo test -p ao-core       # unit tests (no tmux/git needed)
cargo clippy                # lint
cargo fmt --check           # format check
cargo run -p ao-tui         # run the TUI
```

## Key Conventions

- State file: `.slots/state.json` with `camelCase` field names (C# compat)
- Config file: `.aspire-orchestrator.yaml` in repo root
- Tmux sessions: `ao-{slot_name}`
- Log files: `.aspire-orchestrator-aspire.log` and `.aspire-orchestrator-agent.log` in clone dir
- Error handling: `thiserror` in ao-core, `color-eyre` in ao-tui
- Binary imports from library: `main.rs` uses `use ao_tui::` (not `mod` re-declarations) to avoid double compilation
