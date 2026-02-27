mod app;
mod event;
mod keys;
mod ui;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use ao_core::models::RepoCandidate;
use ao_core::services::log_tailer::LogLine;
use ao_core::services::slot_manager::SlotManager;
use ao_core::services::tmux;

use crate::app::{App, CreateSlotField, Mode};
use crate::event::{spawn_input_task, spawn_tick_task, AppEvent};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    // Parse CLI args
    let args: Vec<String> = std::env::args().collect();
    let debug = args.iter().any(|a| a == "--debug");
    let headless_script = args
        .iter()
        .position(|a| a == "--headless")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);

    // Set up debug logging if requested
    let _guard = if debug || headless_script.is_some() {
        Some(setup_debug_logging())
    } else {
        None
    };

    if let Some(script_path) = headless_script {
        run_headless(script_path).await
    } else {
        run_interactive().await
    }
}

/// Configure file-based tracing to `.aspire-orchestrator-debug.log` in CWD.
/// Returns the guard that must be held alive for the duration of the program.
fn setup_debug_logging() -> tracing_appender::non_blocking::WorkerGuard {
    let file_appender = tracing_appender::rolling::never(".", ".aspire-orchestrator-debug.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug")),
        )
        .with_ansi(false)
        .init();

    guard
}

/// Run the normal interactive TUI with crossterm backend.
async fn run_interactive() -> color_eyre::Result<()> {
    let slots_directory = find_slots_directory();
    std::fs::create_dir_all(&slots_directory)?;

    let (log_tx, mut log_rx) = mpsc::unbounded_channel::<LogLine>();
    let slot_manager = Arc::new(SlotManager::new(slots_directory, log_tx));
    slot_manager.load_state().await?;
    slot_manager.reconnect_existing_sessions().await?;

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();
    let _input_task = spawn_input_task(event_tx.clone());
    let _tick_task = spawn_tick_task(event_tx.clone());

    // Forward log lines into the event channel
    let log_event_tx = event_tx.clone();
    tokio::spawn(async move {
        while let Some(log_line) = log_rx.recv().await {
            let _ = log_event_tx.send(AppEvent::Log(log_line));
        }
    });

    // Initialize terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    app.slots = slot_manager.get_slots().await;

    // Main event loop
    loop {
        terminal.draw(|f| ui::render(f, &app, None))?;

        if let Some(session) = app.pop_in_target.take() {
            pop_in(&mut terminal, &session).await;
            app.slots = slot_manager.get_slots().await;
            continue;
        }

        if let Ok(event) = event_rx.try_recv() {
            process_event(&mut app, event, &slot_manager, &event_tx).await;
        } else {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }

        // Debounce check
        if matches!(app.mode, Mode::CreateSlotDialog)
            && app.create_form.focus == CreateSlotField::Source
            && app.create_form.should_filter_now()
        {
            tracing::debug!("filter_applied");
            app.create_form.apply_filter();
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

/// Run headless mode: read scripted input, render to TestBackend, dump frames to stdout.
async fn run_headless(script_path: PathBuf) -> color_eyre::Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use ratatui::backend::TestBackend;

    let script = std::fs::read_to_string(&script_path)?;
    let lines: Vec<&str> = script.lines().collect();

    // Set up real slot manager + event channels
    let slots_directory = find_slots_directory();
    std::fs::create_dir_all(&slots_directory)?;

    let (log_tx, mut log_rx) = mpsc::unbounded_channel::<LogLine>();
    let slot_manager = Arc::new(SlotManager::new(slots_directory, log_tx));
    slot_manager.load_state().await?;
    slot_manager.reconnect_existing_sessions().await?;

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();
    // No spawn_input_task — input comes from script
    let _tick_task = spawn_tick_task(event_tx.clone());

    // Forward log lines
    let log_event_tx = event_tx.clone();
    tokio::spawn(async move {
        while let Some(log_line) = log_rx.recv().await {
            let _ = log_event_tx.send(AppEvent::Log(log_line));
        }
    });

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend)?;
    let mut app = App::new();
    app.slots = slot_manager.get_slots().await;

    for raw_line in &lines {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if line == "quit" {
            break;
        }

        if line == "screenshot" {
            headless_screenshot(&mut terminal, &app, None)?;
            continue;
        }

        if let Some(label) = line.strip_prefix("screenshot:") {
            headless_screenshot(&mut terminal, &app, Some(label.trim()))?;
            continue;
        }

        if line == "inject_candidates" {
            headless_inject_candidates(&mut app);
            continue;
        }

        if let Some(ms_str) = line.strip_prefix("wait:") {
            let ms: u64 = ms_str.trim().parse().unwrap_or(100);
            headless_wait(&mut app, &mut event_rx, &slot_manager, &event_tx, ms).await;
            continue;
        }

        if let Some(text) = line.strip_prefix("type:") {
            for ch in text.chars() {
                let key_event = KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE);
                let key_event = KeyEvent {
                    kind: KeyEventKind::Press,
                    state: KeyEventState::NONE,
                    ..key_event
                };
                keys::handle_key(&mut app, key_event, &slot_manager, &event_tx).await;
            }
            continue;
        }

        if let Some(key_str) = line.strip_prefix("key:") {
            let trimmed = key_str.trim();

            // Check for Shift+ prefix
            let (modifiers, key_name) = if let Some(rest) = trimmed.strip_prefix("shift+") {
                (KeyModifiers::SHIFT, rest)
            } else {
                (KeyModifiers::NONE, trimmed)
            };

            let key_code = match key_name {
                "enter" => KeyCode::Enter,
                "esc" => KeyCode::Esc,
                "tab" => KeyCode::Tab,
                "backtab" => KeyCode::BackTab,
                "up" => KeyCode::Up,
                "down" => KeyCode::Down,
                "left" => KeyCode::Left,
                "right" => KeyCode::Right,
                "backspace" => KeyCode::Backspace,
                "pageup" => KeyCode::PageUp,
                "pagedown" => KeyCode::PageDown,
                s if s.len() == 1 => {
                    let ch = s.chars().next().unwrap();
                    if modifiers.contains(KeyModifiers::SHIFT) {
                        KeyCode::Char(ch.to_uppercase().next().unwrap_or(ch))
                    } else {
                        KeyCode::Char(ch)
                    }
                }
                other => {
                    eprintln!("headless: unknown key '{other}'");
                    continue;
                }
            };

            let final_modifiers = if key_name == "backtab" {
                KeyModifiers::SHIFT
            } else {
                modifiers
            };

            let key_event = KeyEvent {
                code: key_code,
                modifiers: final_modifiers,
                kind: KeyEventKind::Press,
                state: KeyEventState::NONE,
            };
            keys::handle_key(&mut app, key_event, &slot_manager, &event_tx).await;
            continue;
        }

        eprintln!("headless: unknown command '{line}'");
    }

    Ok(())
}

/// Render the current app state to the TestBackend and dump frame text to stdout.
fn headless_screenshot(
    terminal: &mut Terminal<ratatui::backend::TestBackend>,
    app: &App,
    label: Option<&str>,
) -> color_eyre::Result<()> {
    if let Some(label) = label {
        println!("=== {label} ===");
    }
    terminal.draw(|f| ui::render(f, app, None))?;
    let buf = terminal.backend().buffer();
    for y in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            let cell = &buf[(x, y)];
            line.push_str(cell.symbol());
        }
        println!("{}", line.trim_end());
    }
    Ok(())
}

/// Inject hardcoded test repo candidates for headless testing.
fn headless_inject_candidates(app: &mut App) {
    tracing::debug!("inject_candidates");
    let candidates = vec![
        RepoCandidate {
            name: "aspire-orchestrator".into(),
            local_path: Some("C:/Users/test/Source/aspire-orchestrator".into()),
            remote_url: Some("https://github.com/test/aspire-orchestrator".into()),
        },
        RepoCandidate {
            name: "Receipts".into(),
            local_path: Some("C:/Users/test/Source/Receipts".into()),
            remote_url: Some("https://github.com/test/Receipts".into()),
        },
        RepoCandidate {
            name: "dotfiles".into(),
            local_path: Some("C:/Users/test/Source/dotfiles".into()),
            remote_url: None,
        },
        RepoCandidate {
            name: "cloud-api".into(),
            local_path: None,
            remote_url: Some("https://github.com/test/cloud-api".into()),
        },
        RepoCandidate {
            name: "infra-tools".into(),
            local_path: None,
            remote_url: Some("https://github.com/test/infra-tools".into()),
        },
    ];
    app.create_form.all_candidates = candidates;
    app.create_form.scan_loading = false;
    app.create_form.apply_filter();
}

/// Process async events during a `wait:` command, handling debounce and slot refresh.
async fn headless_wait(
    app: &mut App,
    event_rx: &mut mpsc::UnboundedReceiver<AppEvent>,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
    ms: u64,
) {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(ms);
    while tokio::time::Instant::now() < deadline {
        // Drain pending events
        while let Ok(event) = event_rx.try_recv() {
            process_event(app, event, slot_manager, event_tx).await;
        }

        // Debounce check
        if matches!(app.mode, Mode::CreateSlotDialog)
            && app.create_form.focus == CreateSlotField::Source
            && app.create_form.should_filter_now()
        {
            tracing::debug!("filter_applied");
            app.create_form.apply_filter();
        }

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

/// Process a single AppEvent, updating app state accordingly.
async fn process_event(
    app: &mut App,
    event: AppEvent,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    match event {
        AppEvent::Key(key) => {
            if key.kind == crossterm::event::KeyEventKind::Press {
                keys::handle_key(app, key, slot_manager, event_tx).await;
            }
        }
        AppEvent::Tick => {
            app.slots = slot_manager.get_slots().await;
            if !app.slots.is_empty() && app.selected_index >= app.slots.len() {
                app.selected_index = app.slots.len() - 1;
            }
            // Recompute dashboard activity on each tick
            app.recompute_activity();
        }
        AppEvent::Log(log_line) => {
            // Always record activity for dashboard
            app.record_activity(&log_line.slot_name, &log_line.line);

            // Always push to log buffer for multiplexer
            app.log_buffer.push(
                log_line.slot_name.clone(),
                log_line.source,
                log_line.line.clone(),
            );

            // Update single-slot log view if matching
            if let Some(slot) = app.selected_slot() {
                if slot.name == log_line.slot_name {
                    app.log_lines.push(log_line.line);
                    if app.log_auto_follow {
                        app.log_scroll = app.log_lines.len().saturating_sub(1);
                    }
                }
            }
        }
        AppEvent::ProcessExited { slot_name, code } => {
            app.set_status(format!(
                "Process exited for {slot_name} (code: {})",
                code.map_or("unknown".into(), |c: i32| c.to_string())
            ));
            app.slots = slot_manager.get_slots().await;
        }
        AppEvent::Error(msg) => {
            tracing::debug!(error = %msg, "event_error");
            app.set_status(format!("Error: {msg}"));
            if matches!(app.mode, Mode::Loading(_)) {
                app.mode = Mode::SlotList;
            }
        }
        AppEvent::Info(msg) => {
            tracing::debug!(info = %msg, "event_info");
            app.set_status(msg);
            if matches!(app.mode, Mode::Loading(_)) {
                app.mode = Mode::SlotList;
            }
            app.slots = slot_manager.get_slots().await;
        }
        AppEvent::RepoCandidatesLoaded(candidates) => {
            tracing::debug!(count = candidates.len(), "repo_candidates_loaded");
            app.create_form.all_candidates = candidates;
            app.create_form.scan_loading = false;
            app.create_form.apply_filter();
        }
        AppEvent::BlueprintNamesLoaded(names) => {
            app.blueprint_list.names = names;
            app.blueprint_list.loading = false;
            app.blueprint_list.selected = 0;
        }
        AppEvent::BatchProgress {
            completed,
            total,
            current_slot,
            failure,
            done,
        } => {
            if let Some(ref mut progress) = app.batch_progress {
                progress.completed = completed;
                progress.total = total;
                if current_slot.is_some() {
                    progress.current_slot = current_slot;
                }
                if let Some((name, err)) = failure {
                    progress.failures.push((name, err));
                }
                if done {
                    progress.done = true;
                    // Refresh slots
                    app.slots = slot_manager.get_slots().await;
                }
            }
        }
    }
}

/// Pop into a tmux session: leave alternate screen, attach, then restore.
async fn pop_in(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, session: &str) {
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    let _ = tmux::attach_session(session).await;

    let _ = enable_raw_mode();
    let _ = execute!(terminal.backend_mut(), EnterAlternateScreen);
    let _ = terminal.hide_cursor();
    let _ = terminal.clear();
}

/// Find the .slots directory, walking up from CWD to find a project root.
fn find_slots_directory() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let mut dir = cwd.as_path();
    loop {
        let candidate = dir.join(".slots");
        if candidate.exists() {
            return candidate;
        }
        if dir.join(".git").exists() {
            return candidate;
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    cwd.join(".slots")
}
