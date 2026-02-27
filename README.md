# AspireOrchestrator

A terminal UI for managing isolated .NET Aspire development slots, enabling 4–6 concurrent Claude Code agents working on separate features simultaneously.

Each slot is an independent environment with its own git clone, Aspire stack, and Claude Code agent session — all orchestrated from a single ratatui-based TUI.

## How It Works

```
┌──────────────────────────────────────────────────────────┐
│ AspireOrchestrator                                       │
├────────────────────────┬─────────────────────────────────┤
│ Slots                  │ Details                         │
│                        │  Branch: feature/receipts       │
│ > receipts-1  ▶ ●     │  Status: Running                │
│   receipts-2  ▶ ○     │  Agent: Active                  │
│   webapp-1    ■ ●     │  Dashboard: https://...         │
│                        ├─────────────────────────────────┤
│                        │ Agent Log                       │
│                        │ > Reading src/main.rs...        │
│                        │ > Editing component...          │
│                        │ > Running tests...              │
├────────────────────────┴─────────────────────────────────┤
│ [N]ew [S]tart [K]ill [D]estroy [A]gent [R]ebase         │
│ [G]push [P]op-in [L]og toggle [Q]uit                    │
└──────────────────────────────────────────────────────────┘
```

**Slot lifecycle:**

1. **Create** — clones your repo into `.slots/{name}`, checks out a branch, allocates ports, creates a tmux session
2. **Start Aspire** — spawns `dotnet run` as a child process with isolated port assignments, streams logs to the TUI, discovers service URLs automatically
3. **Spawn Agent** — launches Claude Code in the slot's tmux session with a system prompt containing working directory, branch, and discovered service URLs
4. **Pop in** — drops you into the tmux session to interact with the agent directly, then returns to the TUI on detach
5. **Destroy** — kills processes, removes the clone directory, cleans up state

**Process management is hybrid:**

- **Aspire stacks** run as direct child processes via `tokio::process` — stdout/stderr are captured for log display and service discovery
- **Claude Code sessions** run inside tmux — enabling interactive pop-in via `tmux attach-session`

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [tmux](https://github.com/tmux/tmux) — for Claude Code session management
- [.NET SDK](https://dotnet.microsoft.com/) — for running Aspire AppHosts
- [Claude Code](https://docs.anthropic.com/en/docs/claude-code) — the CLI agent that runs in each slot

## Building

```bash
cargo build --release
```

The binary is at `target/release/ao-tui.exe`.

## Usage

Run from the root of a repository that contains `.aspire-orchestrator.yaml`:

```bash
ao-tui
```

### Configuration

Create `.aspire-orchestrator.yaml` in your repo root:

```yaml
apphost: src/MyApp.AppHost/MyApp.AppHost.csproj

setup:
  - dotnet restore MyApp.slnx
  - npm install

port_overrides:
  VITE_PORT: 5173
  API_PORT: 5001
```

| Field | Required | Description |
|-------|----------|-------------|
| `apphost` | Yes | Path to the Aspire AppHost project |
| `setup` | No | Commands to run after cloning (in the tmux session) |
| `port_overrides` | No | Environment variables set to allocated ports before starting Aspire |

### Hotkeys

| Key | Action |
|-----|--------|
| `N` | Create a new slot |
| `S` | Start Aspire for selected slot |
| `K` | Kill (stop) Aspire |
| `D` | Destroy slot (with confirmation) |
| `A` | Spawn a Claude Code agent |
| `R` | Rebase onto origin/master |
| `G` | Git push current branch |
| `P` / `Enter` | Pop into the slot's tmux session |
| `L` | Toggle between agent and Aspire logs |
| `Q` / `Esc` | Quit |
| `j` / `k` / arrows | Navigate slot list |

### State Persistence

Slot state is saved to `.slots/state.json` in camelCase format. On restart, the orchestrator reconnects to existing tmux sessions and restores slot status.

## Architecture

```
crates/
├── ao-core/        # Library: models, services, no UI dependencies
│   ├── models/     # Slot, SlotStatus, AgentStatus, OrchestratorConfig
│   └── services/   # slot_manager, git, tmux, aspire, agent, discovery, ...
└── ao-tui/         # Binary: ratatui TUI
    ├── app.rs      # Application state and mode management
    ├── event.rs    # Async event producers (input, tick, log lines)
    ├── keys.rs     # Keymap dispatch per mode
    └── ui/         # Render functions for each panel and dialog
```

## Development

```bash
cargo test -p ao-core       # Unit tests (no tmux/git needed)
cargo clippy --workspace    # Lint
cargo fmt --check           # Format check
```
