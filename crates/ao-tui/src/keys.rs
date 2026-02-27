use std::path::Path;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc;

use crate::app::{
    App, BatchProgressState, BlueprintSaveField, BlueprintSaveState, ConfirmAction,
    CreateSlotField, LogSource, Mode, SpawnAgentField, ViewMode,
};
use crate::event::AppEvent;

use ao_core::models::{AgentStatus, SlotStatus};
use ao_core::services::agent_host;
use ao_core::services::blueprint::BlueprintStore;
use ao_core::services::slot_manager::SlotManager;

/// Handle a key event, dispatching based on current mode.
pub async fn handle_key(
    app: &mut App,
    key: KeyEvent,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    tracing::debug!(mode = ?app.mode, key = ?key.code, "handle_key");
    match &app.mode {
        Mode::SlotList => match app.view {
            ViewMode::SlotList => handle_slot_list(app, key, slot_manager, event_tx).await,
            ViewMode::Dashboard => handle_dashboard(app, key, slot_manager, event_tx).await,
        },
        Mode::Terminal => handle_terminal(app, key, slot_manager).await,
        Mode::MultiplexLog => handle_multiplex_log(app, key),
        Mode::CreateSlotDialog => handle_create_dialog(app, key, slot_manager, event_tx),
        Mode::SpawnAgentDialog => handle_agent_dialog(app, key, slot_manager, event_tx),
        Mode::ConfirmDialog { .. } => {
            handle_confirm_dialog(app, key, slot_manager, event_tx);
        }
        Mode::HelpDialog => {
            handle_help_dialog(app, key);
        }
        Mode::BlueprintListDialog => {
            handle_blueprint_list_dialog(app, key, slot_manager, event_tx).await;
        }
        Mode::BlueprintSaveDialog => {
            handle_blueprint_save_dialog(app, key, slot_manager, event_tx);
        }
        Mode::BatchProgress => {
            handle_batch_progress(app, key);
        }
        Mode::Loading(_) => {}
    }
}

// ─── Slot List Mode ─────────────────────────────────────────────────────

async fn handle_slot_list(
    app: &mut App,
    key: KeyEvent,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Check for Shift+key batch operations
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Char('S') => {
                launch_batch_start_all(app, slot_manager, event_tx);
                return;
            }
            KeyCode::Char('K') => {
                launch_batch_stop_all(app, slot_manager, event_tx);
                return;
            }
            KeyCode::Char('R') => {
                launch_batch_rebase_all(app, slot_manager, event_tx);
                return;
            }
            KeyCode::Char('G') => {
                launch_batch_push_all(app, slot_manager, event_tx);
                return;
            }
            KeyCode::Char('D') => {
                if !app.slots.is_empty() {
                    app.mode = Mode::ConfirmDialog {
                        message: format!(
                            "Destroy ALL {} slots? This deletes all clone directories.",
                            app.slots.len()
                        ),
                        action: ConfirmAction::DestroyAll,
                    };
                }
                return;
            }
            KeyCode::Char('A') => {
                launch_batch_spawn_agents(app, slot_manager, event_tx);
                return;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            if app.slots.is_empty() {
                app.should_quit = true;
            } else {
                app.mode = Mode::ConfirmDialog {
                    message: "Quit orchestrator? Running agents will continue in background."
                        .into(),
                    action: ConfirmAction::Quit,
                };
            }
        }
        KeyCode::Up => {
            app.select_prev();
            reload_log_for_selected(app);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.select_next();
            reload_log_for_selected(app);
        }
        KeyCode::Tab => {
            app.view = ViewMode::Dashboard;
            app.dashboard_selected = app.selected_index.min(app.slots.len().saturating_sub(1));
        }
        KeyCode::Char('m') => {
            app.mode = Mode::MultiplexLog;
            app.multiplex_auto_follow = true;
        }
        KeyCode::Char('b') => {
            open_blueprint_list(app, slot_manager, event_tx);
        }
        KeyCode::Char('n') => {
            app.create_form = Default::default();
            app.mode = Mode::CreateSlotDialog;

            let tx = event_tx.clone();
            let workspace_root = slot_manager.workspace_root().to_path_buf();
            tokio::spawn(async move {
                let candidates = ao_core::services::repo_finder::find_repos(&workspace_root).await;
                let _ = tx.send(AppEvent::RepoCandidatesLoaded(candidates));
            });
        }
        KeyCode::Char('s') => {
            if let Some(slot) = app.selected_slot() {
                let name = slot.name.clone();
                let tx = event_tx.clone();
                let sm = Arc::clone(slot_manager);
                tokio::spawn(async move {
                    match sm.start_aspire(&name).await {
                        Ok(()) => {
                            let _ = tx.send(AppEvent::Info(format!("Aspire started for {name}")));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::Error(format!("Start failed: {e}")));
                        }
                    }
                });
            }
        }
        KeyCode::Char('k') => {
            if let Some(slot) = app.selected_slot() {
                let name = slot.name.clone();
                let tx = event_tx.clone();
                let sm = Arc::clone(slot_manager);
                tokio::spawn(async move {
                    match sm.stop_aspire(&name).await {
                        Ok(()) => {
                            let _ = tx.send(AppEvent::Info(format!("Aspire stopped for {name}")));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::Error(format!("Stop failed: {e}")));
                        }
                    }
                });
            }
        }
        KeyCode::Char('d') => {
            if let Some(slot) = app.selected_slot() {
                app.mode = Mode::ConfirmDialog {
                    message: format!(
                        "Destroy slot '{}'? This deletes the clone directory.",
                        slot.name
                    ),
                    action: ConfirmAction::DestroySlot(slot.name.clone()),
                };
            }
        }
        KeyCode::Char('a') => {
            if app.selected_slot().is_some() {
                app.agent_form = Default::default();
                app.mode = Mode::SpawnAgentDialog;
            }
        }
        KeyCode::Char('r') => {
            if let Some(slot) = app.selected_slot() {
                let name = slot.name.clone();
                let tx = event_tx.clone();
                let sm = Arc::clone(slot_manager);
                tokio::spawn(async move {
                    match sm.rebase(&name).await {
                        Ok(()) => {
                            let _ = tx.send(AppEvent::Info(format!("Rebased {name}")));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::Error(format!("Rebase failed: {e}")));
                        }
                    }
                });
            }
        }
        KeyCode::Char('g') => {
            if let Some(slot) = app.selected_slot() {
                let name = slot.name.clone();
                let tx = event_tx.clone();
                let sm = Arc::clone(slot_manager);
                tokio::spawn(async move {
                    match sm.git_push(&name).await {
                        Ok(()) => {
                            let _ = tx.send(AppEvent::Info(format!("Pushed {name}")));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::Error(format!("Push failed: {e}")));
                        }
                    }
                });
            }
        }
        KeyCode::Char('p') | KeyCode::Enter => {
            if let Some(slot) = app.selected_slot() {
                if slot.agent_status == AgentStatus::Active
                    || app.terminal_parsers.contains_key(&slot.name)
                {
                    app.mode = Mode::Terminal;
                }
            }
        }
        KeyCode::Char('l') => {
            app.toggle_log_source();
            reload_log_for_selected(app);
        }
        KeyCode::Char('?') => {
            app.mode = Mode::HelpDialog;
        }
        _ => {}
    }
}

// ─── Dashboard Mode ─────────────────────────────────────────────────────

async fn handle_dashboard(
    app: &mut App,
    key: KeyEvent,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Check for Shift+key batch operations
    if key.modifiers.contains(KeyModifiers::SHIFT) {
        match key.code {
            KeyCode::Char('S') => {
                launch_batch_start_all(app, slot_manager, event_tx);
                return;
            }
            KeyCode::Char('K') => {
                launch_batch_stop_all(app, slot_manager, event_tx);
                return;
            }
            KeyCode::Char('R') => {
                launch_batch_rebase_all(app, slot_manager, event_tx);
                return;
            }
            KeyCode::Char('G') => {
                launch_batch_push_all(app, slot_manager, event_tx);
                return;
            }
            KeyCode::Char('D') => {
                if !app.slots.is_empty() {
                    app.mode = Mode::ConfirmDialog {
                        message: format!(
                            "Destroy ALL {} slots? This deletes all clone directories.",
                            app.slots.len()
                        ),
                        action: ConfirmAction::DestroyAll,
                    };
                }
                return;
            }
            KeyCode::Char('A') => {
                launch_batch_spawn_agents(app, slot_manager, event_tx);
                return;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            if app.slots.is_empty() {
                app.should_quit = true;
            } else {
                app.mode = Mode::ConfirmDialog {
                    message: "Quit orchestrator? Running agents will continue in background."
                        .into(),
                    action: ConfirmAction::Quit,
                };
            }
        }
        KeyCode::Tab => {
            app.view = ViewMode::SlotList;
            if app.dashboard_selected < app.slots.len() {
                app.selected_index = app.dashboard_selected;
            }
            reload_log_for_selected(app);
        }
        KeyCode::Left => app.dashboard_move(-1, 0),
        KeyCode::Right => app.dashboard_move(1, 0),
        KeyCode::Up => app.dashboard_move(0, -1),
        KeyCode::Down | KeyCode::Char('j') => app.dashboard_move(0, 1),
        KeyCode::Enter => {
            app.view = ViewMode::SlotList;
            if app.dashboard_selected < app.slots.len() {
                app.selected_index = app.dashboard_selected;
            }
            reload_log_for_selected(app);
        }
        KeyCode::Char('n') => {
            app.create_form = Default::default();
            app.mode = Mode::CreateSlotDialog;

            let tx = event_tx.clone();
            let workspace_root = slot_manager.workspace_root().to_path_buf();
            tokio::spawn(async move {
                let candidates = ao_core::services::repo_finder::find_repos(&workspace_root).await;
                let _ = tx.send(AppEvent::RepoCandidatesLoaded(candidates));
            });
        }
        KeyCode::Char('m') => {
            app.mode = Mode::MultiplexLog;
            app.multiplex_auto_follow = true;
        }
        KeyCode::Char('b') => {
            open_blueprint_list(app, slot_manager, event_tx);
        }
        KeyCode::Char('p') => {
            if let Some(slot) = app.slots.get(app.dashboard_selected) {
                if slot.agent_status == AgentStatus::Active
                    || app.terminal_parsers.contains_key(&slot.name)
                {
                    app.selected_index = app.dashboard_selected;
                    app.mode = Mode::Terminal;
                }
            }
        }
        KeyCode::Char('?') => {
            app.mode = Mode::HelpDialog;
        }
        _ => {}
    }
}

// ─── Terminal Mode ──────────────────────────────────────────────────────

async fn handle_terminal(app: &mut App, key: KeyEvent, slot_manager: &Arc<SlotManager>) {
    // Esc exits terminal mode
    if key.code == KeyCode::Esc {
        app.mode = Mode::SlotList;
        return;
    }

    // Forward all other keys to the agent PTY
    let slot_name = match app.selected_slot() {
        Some(s) => s.name.clone(),
        None => {
            app.mode = Mode::SlotList;
            return;
        }
    };

    // Get or create connection
    if !app.agent_connections.contains_key(&slot_name) {
        let slots_dir = slot_manager.slots_directory().to_path_buf();
        match agent_host::connect(&slot_name, &slots_dir).await {
            Ok(conn) => {
                app.agent_connections
                    .insert(slot_name.clone(), Arc::new(tokio::sync::Mutex::new(conn)));
            }
            Err(_) => {
                app.set_status(format!("Cannot connect to agent for {slot_name}"));
                app.mode = Mode::SlotList;
                return;
            }
        }
    }

    let bytes = key_to_bytes(key);
    if !bytes.is_empty() {
        if let Some(conn) = app.agent_connections.get(&slot_name) {
            let mut conn = conn.lock().await;
            let _ = conn.send_input(&bytes).await;
        }
    }
}

/// Convert a crossterm KeyEvent to terminal escape bytes.
fn key_to_bytes(key: KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                // Ctrl+A = 0x01, Ctrl+C = 0x03, etc.
                let ctrl = (c as u8).wrapping_sub(b'a').wrapping_add(1);
                vec![ctrl]
            } else {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                s.as_bytes().to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Tab => vec![b'\t'],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        KeyCode::Home => b"\x1b[H".to_vec(),
        KeyCode::End => b"\x1b[F".to_vec(),
        KeyCode::PageUp => b"\x1b[5~".to_vec(),
        KeyCode::PageDown => b"\x1b[6~".to_vec(),
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Insert => b"\x1b[2~".to_vec(),
        KeyCode::F(n) => match n {
            1 => b"\x1bOP".to_vec(),
            2 => b"\x1bOQ".to_vec(),
            3 => b"\x1bOR".to_vec(),
            4 => b"\x1bOS".to_vec(),
            5 => b"\x1b[15~".to_vec(),
            6 => b"\x1b[17~".to_vec(),
            7 => b"\x1b[18~".to_vec(),
            8 => b"\x1b[19~".to_vec(),
            9 => b"\x1b[20~".to_vec(),
            10 => b"\x1b[21~".to_vec(),
            11 => b"\x1b[23~".to_vec(),
            12 => b"\x1b[24~".to_vec(),
            _ => vec![],
        },
        _ => vec![],
    }
}

// ─── Multiplex Log Mode ────────────────────────────────────────────────

fn handle_multiplex_log(app: &mut App, key: KeyEvent) {
    // Handle search input mode
    if app.multiplex_filter.search_input_active {
        match key.code {
            KeyCode::Esc => {
                app.multiplex_filter.search_input_active = false;
            }
            KeyCode::Enter => {
                app.multiplex_filter.search_filter_mode = !app.multiplex_filter.search_filter_mode;
                app.multiplex_filter.search_input_active = false;
            }
            KeyCode::Backspace => {
                app.multiplex_filter.search_text.pop();
                app.multiplex_filter.update_regex();
            }
            KeyCode::Char(c) => {
                app.multiplex_filter.search_text.push(c);
                app.multiplex_filter.update_regex();
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('m') | KeyCode::Esc => {
            app.mode = Mode::SlotList;
        }
        KeyCode::Char('1')
        | KeyCode::Char('2')
        | KeyCode::Char('3')
        | KeyCode::Char('4')
        | KeyCode::Char('5')
        | KeyCode::Char('6') => {
            let idx = match key.code {
                KeyCode::Char(c) => c.to_digit(10).unwrap_or(0) as usize - 1,
                _ => return,
            };
            if let Some(slot) = app.slots.get(idx) {
                let name = slot.name.clone();
                if app.multiplex_filter.hidden_slots.contains(&name) {
                    app.multiplex_filter.hidden_slots.remove(&name);
                } else {
                    app.multiplex_filter.hidden_slots.insert(name);
                }
            }
        }
        KeyCode::Tab => {
            app.multiplex_filter.cycle_source();
        }
        KeyCode::Char('/') => {
            app.multiplex_filter.search_input_active = true;
        }
        KeyCode::Char('c') => {
            app.multiplex_filter.search_text.clear();
            app.multiplex_filter.search_regex = None;
            app.multiplex_filter.search_filter_mode = false;
        }
        KeyCode::Char('e') => {
            // Jump to next error
            jump_to_error(app, true);
        }
        KeyCode::Char('w') => {
            // Jump to prev error
            jump_to_error(app, false);
        }
        KeyCode::Char('f') => {
            app.multiplex_auto_follow = true;
        }
        KeyCode::Up => {
            app.multiplex_auto_follow = false;
            app.multiplex_scroll = app.multiplex_scroll.saturating_sub(1);
        }
        KeyCode::Down => {
            app.multiplex_auto_follow = false;
            app.multiplex_scroll += 1;
        }
        KeyCode::PageUp => {
            app.multiplex_auto_follow = false;
            app.multiplex_scroll = app.multiplex_scroll.saturating_sub(20);
        }
        KeyCode::PageDown => {
            app.multiplex_auto_follow = false;
            app.multiplex_scroll += 20;
        }
        KeyCode::Enter => {
            // Jump to selected slot's detail view based on current scroll position
            if let Some(entry) = get_entry_at_scroll(app) {
                let name = entry.to_string();
                if let Some(idx) = app.slots.iter().position(|s| s.name == name) {
                    app.selected_index = idx;
                    app.mode = Mode::SlotList;
                    app.view = ViewMode::SlotList;
                    reload_log_for_selected(app);
                }
            }
        }
        _ => {}
    }
}

fn jump_to_error(app: &mut App, forward: bool) {
    use crate::app::Severity;

    let filtered: Vec<usize> = app
        .log_buffer
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            app.multiplex_filter.matches_entry(e)
                && matches!(e.severity, Severity::Error | Severity::Warn)
        })
        .map(|(i, _)| i)
        .collect();

    if filtered.is_empty() {
        return;
    }

    let current_scroll = app.multiplex_scroll;
    if forward {
        if let Some(&idx) = filtered.iter().find(|&&i| i > current_scroll) {
            app.multiplex_scroll = idx;
            app.multiplex_auto_follow = false;
        }
    } else if let Some(&idx) = filtered.iter().rev().find(|&&i| i < current_scroll) {
        app.multiplex_scroll = idx;
        app.multiplex_auto_follow = false;
    }
}

fn get_entry_at_scroll(app: &App) -> Option<String> {
    let filtered: Vec<&crate::app::LogEntry> = app
        .log_buffer
        .entries
        .iter()
        .filter(|e| app.multiplex_filter.matches_entry(e))
        .collect();

    filtered
        .get(app.multiplex_scroll)
        .map(|e| e.slot_name.clone())
}

// ─── Blueprint List Dialog ──────────────────────────────────────────────

async fn handle_blueprint_list_dialog(
    app: &mut App,
    key: KeyEvent,
    slot_manager: &Arc<SlotManager>,
    _event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::SlotList;
        }
        KeyCode::Up => {
            if app.blueprint_list.selected > 0 {
                app.blueprint_list.selected -= 1;
            }
        }
        KeyCode::Down => {
            if app.blueprint_list.selected + 1 < app.blueprint_list.names.len() {
                app.blueprint_list.selected += 1;
            }
        }
        KeyCode::Char('s') => {
            app.blueprint_save = BlueprintSaveState::new();
            app.mode = Mode::BlueprintSaveDialog;
        }
        KeyCode::Char('d') => {
            if let Some(name) = app
                .blueprint_list
                .names
                .get(app.blueprint_list.selected)
                .cloned()
            {
                let store = BlueprintStore::new(&slot_manager.workspace_root().join(".slots"));
                match store.delete(&name).await {
                    Ok(()) => {
                        app.blueprint_list.names.retain(|n| n != &name);
                        if app.blueprint_list.selected >= app.blueprint_list.names.len()
                            && app.blueprint_list.selected > 0
                        {
                            app.blueprint_list.selected -= 1;
                        }
                        app.set_status(format!("Deleted blueprint '{name}'"));
                    }
                    Err(e) => {
                        app.set_status(format!("Delete failed: {e}"));
                    }
                }
            }
        }
        KeyCode::Enter => {
            if let Some(name) = app
                .blueprint_list
                .names
                .get(app.blueprint_list.selected)
                .cloned()
            {
                app.mode = Mode::ConfirmDialog {
                    message: format!("Load blueprint '{name}'? This will create new slots."),
                    action: ConfirmAction::LoadBlueprint(name),
                };
            }
        }
        _ => {}
    }
}

// ─── Blueprint Save Dialog ──────────────────────────────────────────────

fn handle_blueprint_save_dialog(
    app: &mut App,
    key: KeyEvent,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::BlueprintListDialog;
        }
        KeyCode::Tab | KeyCode::BackTab => {
            app.blueprint_save.focus = match app.blueprint_save.focus {
                BlueprintSaveField::Name => BlueprintSaveField::Description,
                BlueprintSaveField::Description => BlueprintSaveField::Name,
            };
        }
        KeyCode::Enter => {
            let name = app.blueprint_save.name.trim().to_string();
            if name.is_empty() {
                app.set_status("Blueprint name is required");
                return;
            }
            let desc = app.blueprint_save.description.trim().to_string();
            let desc_opt = if desc.is_empty() { None } else { Some(desc) };

            let slots: Vec<ao_core::models::Slot> = app.slots.clone();
            let store = BlueprintStore::new(&slot_manager.workspace_root().join(".slots"));
            let bp = BlueprintStore::snapshot_from_slots(&name, desc_opt.as_deref(), &slots);

            let tx = event_tx.clone();
            app.mode = Mode::Loading(format!("Saving blueprint '{name}'..."));

            tokio::spawn(async move {
                match store.save(&bp).await {
                    Ok(()) => {
                        let _ = tx.send(AppEvent::Info(format!("Saved blueprint '{name}'")));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::Error(format!("Save failed: {e}")));
                    }
                }
            });
        }
        KeyCode::Backspace => match app.blueprint_save.focus {
            BlueprintSaveField::Name => {
                app.blueprint_save.name.pop();
            }
            BlueprintSaveField::Description => {
                app.blueprint_save.description.pop();
            }
        },
        KeyCode::Char(c) => match app.blueprint_save.focus {
            BlueprintSaveField::Name => app.blueprint_save.name.push(c),
            BlueprintSaveField::Description => app.blueprint_save.description.push(c),
        },
        _ => {}
    }
}

// ─── Batch Progress ────────────────────────────────────────────────────

fn handle_batch_progress(app: &mut App, key: KeyEvent) {
    if let Some(ref progress) = app.batch_progress {
        if progress.done {
            match key.code {
                KeyCode::Esc | KeyCode::Enter => {
                    app.batch_progress = None;
                    app.mode = Mode::SlotList;
                }
                _ => {}
            }
        }
    }
}

// ─── Confirm Dialog ────────────────────────────────────────────────────

fn handle_confirm_dialog(
    app: &mut App,
    key: KeyEvent,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('n') => {
            app.mode = Mode::SlotList;
        }
        KeyCode::Char('y') | KeyCode::Enter => {
            let Mode::ConfirmDialog { action, .. } = &app.mode else {
                return;
            };
            match action.clone() {
                ConfirmAction::Quit => {
                    app.should_quit = true;
                }
                ConfirmAction::DestroySlot(name) => {
                    let tx = event_tx.clone();
                    let sm = Arc::clone(slot_manager);
                    app.mode = Mode::Loading(format!("Destroying {name}..."));
                    tokio::spawn(async move {
                        match sm.destroy_slot(&name).await {
                            Ok(()) => {
                                let _ = tx.send(AppEvent::Info(format!("Destroyed slot {name}")));
                            }
                            Err(e) => {
                                let _ = tx.send(AppEvent::Error(format!("Destroy failed: {e}")));
                            }
                        }
                    });
                }
                ConfirmAction::DestroyAll => {
                    launch_batch_destroy_all(app, slot_manager, event_tx);
                }
                ConfirmAction::LoadBlueprint(name) => {
                    launch_blueprint_load(app, &name, slot_manager, event_tx);
                }
            }
        }
        _ => {}
    }
}

// ─── Other Dialog Handlers ──────────────────────────────────────────────

fn handle_create_dialog(
    app: &mut App,
    key: KeyEvent,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::SlotList;
        }
        KeyCode::Tab | KeyCode::BackTab => {
            app.create_form.focus = match app.create_form.focus {
                CreateSlotField::Source => {
                    app.create_form.filtered_candidates.clear();
                    app.create_form.selected_candidate = None;
                    CreateSlotField::Prompt
                }
                CreateSlotField::Prompt => {
                    app.create_form.apply_filter();
                    CreateSlotField::Source
                }
            };
        }
        KeyCode::Up => {
            if app.create_form.focus == CreateSlotField::Source
                && !app.create_form.filtered_candidates.is_empty()
            {
                app.create_form.select_prev_candidate();
            }
        }
        KeyCode::Down => {
            if app.create_form.focus == CreateSlotField::Source
                && !app.create_form.filtered_candidates.is_empty()
            {
                app.create_form.select_next_candidate();
            }
        }
        KeyCode::Enter => {
            if app.create_form.focus == CreateSlotField::Source
                && app.create_form.selected_candidate.is_some()
            {
                app.create_form.accept_selected();
                return;
            }

            let source = app.create_form.source.trim().to_string();
            if source.is_empty() {
                app.set_status("Source path is required");
                return;
            }
            let prompt = app.create_form.prompt.trim().to_string();
            let prompt_opt = if prompt.is_empty() {
                None
            } else {
                Some(prompt)
            };

            let name = Path::new(&source)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "slot".into());

            let tx = event_tx.clone();
            let sm = Arc::clone(slot_manager);
            app.mode = Mode::Loading(format!("Creating slot {name}..."));

            tokio::spawn(async move {
                match sm
                    .create_slot(&name, &source, None, prompt_opt.as_deref())
                    .await
                {
                    Ok(slot) => {
                        let _ = tx.send(AppEvent::Info(format!("Created slot '{}'", slot.name)));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::Error(format!("Create failed: {e}")));
                    }
                }
            });
        }
        KeyCode::Backspace => match app.create_form.focus {
            CreateSlotField::Source => {
                app.create_form.source.pop();
                app.create_form.schedule_filter();
            }
            CreateSlotField::Prompt => {
                app.create_form.prompt.pop();
            }
        },
        KeyCode::Char(c) => match app.create_form.focus {
            CreateSlotField::Source => {
                app.create_form.source.push(c);
                app.create_form.schedule_filter();
            }
            CreateSlotField::Prompt => app.create_form.prompt.push(c),
        },
        _ => {}
    }
}

fn handle_agent_dialog(
    app: &mut App,
    key: KeyEvent,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    match key.code {
        KeyCode::Esc => {
            app.mode = Mode::SlotList;
        }
        KeyCode::Tab => {
            app.agent_form.focus = match app.agent_form.focus {
                SpawnAgentField::Prompt => SpawnAgentField::AllowedTools,
                SpawnAgentField::AllowedTools => SpawnAgentField::MaxTurns,
                SpawnAgentField::MaxTurns => SpawnAgentField::Prompt,
            };
        }
        KeyCode::BackTab => {
            app.agent_form.focus = match app.agent_form.focus {
                SpawnAgentField::Prompt => SpawnAgentField::MaxTurns,
                SpawnAgentField::AllowedTools => SpawnAgentField::Prompt,
                SpawnAgentField::MaxTurns => SpawnAgentField::AllowedTools,
            };
        }
        KeyCode::Enter => {
            let Some(slot) = app.selected_slot() else {
                return;
            };
            let name = slot.name.clone();
            let prompt = app.agent_form.prompt.trim().to_string();
            let tools = app.agent_form.allowed_tools.trim().to_string();
            let max_turns: Option<u32> = app.agent_form.max_turns.trim().parse().ok();

            let prompt_opt = if prompt.is_empty() {
                None
            } else {
                Some(prompt)
            };
            let tools_opt = if tools.is_empty() { None } else { Some(tools) };

            let tx = event_tx.clone();
            let sm = Arc::clone(slot_manager);
            app.mode = Mode::Loading(format!("Spawning agent for {name}..."));

            tokio::spawn(async move {
                match sm
                    .spawn_agent(
                        &name,
                        prompt_opt.as_deref(),
                        tools_opt.as_deref(),
                        max_turns,
                    )
                    .await
                {
                    Ok(()) => {
                        let _ = tx.send(AppEvent::Info(format!("Agent spawned for {name}")));
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::Error(format!("Spawn failed: {e}")));
                    }
                }
            });
        }
        KeyCode::Backspace => match app.agent_form.focus {
            SpawnAgentField::Prompt => {
                app.agent_form.prompt.pop();
            }
            SpawnAgentField::AllowedTools => {
                app.agent_form.allowed_tools.pop();
            }
            SpawnAgentField::MaxTurns => {
                app.agent_form.max_turns.pop();
            }
        },
        KeyCode::Char(c) => match app.agent_form.focus {
            SpawnAgentField::Prompt => app.agent_form.prompt.push(c),
            SpawnAgentField::AllowedTools => app.agent_form.allowed_tools.push(c),
            SpawnAgentField::MaxTurns => {
                if c.is_ascii_digit() {
                    app.agent_form.max_turns.push(c);
                }
            }
        },
        _ => {}
    }
}

fn handle_help_dialog(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
            app.mode = Mode::SlotList;
        }
        _ => {}
    }
}

// ─── Helper: Reload Log ─────────────────────────────────────────────────

fn reload_log_for_selected(app: &mut App) {
    app.log_lines.clear();
    app.log_scroll = 0;

    if let Some(slot) = app.selected_slot() {
        let log_path = match app.log_source {
            LogSource::Agent => slot.agent_log_path(),
            LogSource::Aspire => slot.aspire_log_path(),
        };
        let tail = ao_core::services::log_tailer::read_tail(&log_path, 50);
        app.log_lines = tail.lines().map(|l| l.to_string()).collect();
        if app.log_auto_follow {
            app.log_scroll = app.log_lines.len().saturating_sub(1);
        }
    }
}

// ─── Blueprint Helpers ──────────────────────────────────────────────────

fn open_blueprint_list(
    app: &mut App,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    app.blueprint_list.loading = true;
    app.blueprint_list.names.clear();
    app.blueprint_list.selected = 0;
    app.mode = Mode::BlueprintListDialog;

    let tx = event_tx.clone();
    let slots_dir = slot_manager.workspace_root().join(".slots");
    tokio::spawn(async move {
        let store = BlueprintStore::new(&slots_dir);
        let names = store.list().await.unwrap_or_default();
        let _ = tx.send(AppEvent::BlueprintNamesLoaded(names));
    });
}

fn launch_blueprint_load(
    app: &mut App,
    name: &str,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    let store = BlueprintStore::new(&slot_manager.workspace_root().join(".slots"));
    let name = name.to_string();
    let tx = event_tx.clone();
    let sm = Arc::clone(slot_manager);

    app.mode = Mode::BatchProgress;
    app.batch_progress = Some(BatchProgressState::new("Loading Blueprint", 0));

    tokio::spawn(async move {
        // Load and resolve
        let blueprint = match store.load(&name).await {
            Ok(bp) => bp,
            Err(e) => {
                let _ = tx.send(AppEvent::Error(format!("Load failed: {e}")));
                let _ = tx.send(AppEvent::BatchProgress {
                    completed: 0,
                    total: 0,
                    current_slot: None,
                    failure: None,
                    done: true,
                });
                return;
            }
        };

        let resolved = match ao_core::services::blueprint::resolve(&blueprint) {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(AppEvent::Error(format!("Resolve failed: {e}")));
                let _ = tx.send(AppEvent::BatchProgress {
                    completed: 0,
                    total: 0,
                    current_slot: None,
                    failure: None,
                    done: true,
                });
                return;
            }
        };

        let total = resolved.len();
        let _ = tx.send(AppEvent::BatchProgress {
            completed: 0,
            total,
            current_slot: None,
            failure: None,
            done: false,
        });

        for (i, slot_config) in resolved.iter().enumerate() {
            let _ = tx.send(AppEvent::BatchProgress {
                completed: i,
                total,
                current_slot: Some(slot_config.name.clone()),
                failure: None,
                done: false,
            });

            let prompt = if slot_config.auto_spawn_agent {
                slot_config.prompt.as_deref()
            } else {
                None
            };

            match sm
                .create_slot(
                    &slot_config.name,
                    &slot_config.source,
                    slot_config.branch.as_deref(),
                    prompt,
                )
                .await
            {
                Ok(_) => {
                    if slot_config.auto_start_aspire {
                        if let Err(e) = sm.start_aspire(&slot_config.name).await {
                            let _ = tx.send(AppEvent::BatchProgress {
                                completed: i + 1,
                                total,
                                current_slot: None,
                                failure: Some((
                                    slot_config.name.clone(),
                                    format!("Aspire start: {e}"),
                                )),
                                done: false,
                            });
                        }
                        // Stagger delay
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::BatchProgress {
                        completed: i + 1,
                        total,
                        current_slot: None,
                        failure: Some((slot_config.name.clone(), e.to_string())),
                        done: false,
                    });
                }
            }
        }

        let _ = tx.send(AppEvent::BatchProgress {
            completed: total,
            total,
            current_slot: None,
            failure: None,
            done: true,
        });
        let _ = tx.send(AppEvent::Info(format!(
            "Blueprint '{name}' loaded ({total} slots)"
        )));
    });
}

// ─── Batch Operation Launchers ──────────────────────────────────────────

fn launch_batch_start_all(
    app: &mut App,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    let targets: Vec<String> = app
        .slots
        .iter()
        .filter(|s| s.status == SlotStatus::Ready)
        .map(|s| s.name.clone())
        .collect();

    if targets.is_empty() {
        app.set_status("No slots in Ready state to start");
        return;
    }

    let total = targets.len();
    app.batch_progress = Some(BatchProgressState::new("Start All Aspire", total));
    app.mode = Mode::BatchProgress;

    let sm = Arc::clone(slot_manager);
    let tx = event_tx.clone();

    tokio::spawn(async move {
        for (i, name) in targets.iter().enumerate() {
            let _ = tx.send(AppEvent::BatchProgress {
                completed: i,
                total,
                current_slot: Some(name.clone()),
                failure: None,
                done: false,
            });

            if let Err(e) = sm.start_aspire(name).await {
                let _ = tx.send(AppEvent::BatchProgress {
                    completed: i + 1,
                    total,
                    current_slot: None,
                    failure: Some((name.clone(), e.to_string())),
                    done: false,
                });
            }

            // Stagger delay
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        let _ = tx.send(AppEvent::BatchProgress {
            completed: total,
            total,
            current_slot: None,
            failure: None,
            done: true,
        });
    });
}

fn launch_batch_stop_all(
    app: &mut App,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    let targets: Vec<String> = app
        .slots
        .iter()
        .filter(|s| s.status == SlotStatus::Running)
        .map(|s| s.name.clone())
        .collect();

    if targets.is_empty() {
        app.set_status("No running slots to stop");
        return;
    }

    let total = targets.len();
    app.batch_progress = Some(BatchProgressState::new("Stop All Aspire", total));
    app.mode = Mode::BatchProgress;

    let sm = Arc::clone(slot_manager);
    let tx = event_tx.clone();

    tokio::spawn(async move {
        for (i, name) in targets.iter().enumerate() {
            let _ = tx.send(AppEvent::BatchProgress {
                completed: i,
                total,
                current_slot: Some(name.clone()),
                failure: None,
                done: false,
            });

            if let Err(e) = sm.stop_aspire(name).await {
                let _ = tx.send(AppEvent::BatchProgress {
                    completed: i + 1,
                    total,
                    current_slot: None,
                    failure: Some((name.clone(), e.to_string())),
                    done: false,
                });
            }
        }

        let _ = tx.send(AppEvent::BatchProgress {
            completed: total,
            total,
            current_slot: None,
            failure: None,
            done: true,
        });
    });
}

fn launch_batch_rebase_all(
    app: &mut App,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    let targets: Vec<String> = app.slots.iter().map(|s| s.name.clone()).collect();

    if targets.is_empty() {
        app.set_status("No slots to rebase");
        return;
    }

    let total = targets.len();
    app.batch_progress = Some(BatchProgressState::new("Rebase All", total));
    app.mode = Mode::BatchProgress;

    let sm = Arc::clone(slot_manager);
    let tx = event_tx.clone();

    tokio::spawn(async move {
        for (i, name) in targets.iter().enumerate() {
            let _ = tx.send(AppEvent::BatchProgress {
                completed: i,
                total,
                current_slot: Some(name.clone()),
                failure: None,
                done: false,
            });

            if let Err(e) = sm.rebase(name).await {
                let _ = tx.send(AppEvent::BatchProgress {
                    completed: i + 1,
                    total,
                    current_slot: None,
                    failure: Some((name.clone(), e.to_string())),
                    done: false,
                });
            }
        }

        let _ = tx.send(AppEvent::BatchProgress {
            completed: total,
            total,
            current_slot: None,
            failure: None,
            done: true,
        });
    });
}

fn launch_batch_push_all(
    app: &mut App,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    let targets: Vec<String> = app.slots.iter().map(|s| s.name.clone()).collect();

    if targets.is_empty() {
        app.set_status("No slots to push");
        return;
    }

    let total = targets.len();
    app.batch_progress = Some(BatchProgressState::new("Push All", total));
    app.mode = Mode::BatchProgress;

    let sm = Arc::clone(slot_manager);
    let tx = event_tx.clone();

    tokio::spawn(async move {
        for (i, name) in targets.iter().enumerate() {
            let _ = tx.send(AppEvent::BatchProgress {
                completed: i,
                total,
                current_slot: Some(name.clone()),
                failure: None,
                done: false,
            });

            if let Err(e) = sm.git_push(name).await {
                let _ = tx.send(AppEvent::BatchProgress {
                    completed: i + 1,
                    total,
                    current_slot: None,
                    failure: Some((name.clone(), e.to_string())),
                    done: false,
                });
            }
        }

        let _ = tx.send(AppEvent::BatchProgress {
            completed: total,
            total,
            current_slot: None,
            failure: None,
            done: true,
        });
    });
}

fn launch_batch_destroy_all(
    app: &mut App,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    let targets: Vec<String> = app.slots.iter().map(|s| s.name.clone()).collect();

    if targets.is_empty() {
        app.set_status("No slots to destroy");
        return;
    }

    let total = targets.len();
    app.batch_progress = Some(BatchProgressState::new("Destroy All", total));
    app.mode = Mode::BatchProgress;

    let sm = Arc::clone(slot_manager);
    let tx = event_tx.clone();

    tokio::spawn(async move {
        for (i, name) in targets.iter().enumerate() {
            let _ = tx.send(AppEvent::BatchProgress {
                completed: i,
                total,
                current_slot: Some(name.clone()),
                failure: None,
                done: false,
            });

            if let Err(e) = sm.destroy_slot(name).await {
                let _ = tx.send(AppEvent::BatchProgress {
                    completed: i + 1,
                    total,
                    current_slot: None,
                    failure: Some((name.clone(), e.to_string())),
                    done: false,
                });
            }
        }

        let _ = tx.send(AppEvent::BatchProgress {
            completed: total,
            total,
            current_slot: None,
            failure: None,
            done: true,
        });
    });
}

fn launch_batch_spawn_agents(
    app: &mut App,
    slot_manager: &Arc<SlotManager>,
    event_tx: &mpsc::UnboundedSender<AppEvent>,
) {
    let targets: Vec<String> = app
        .slots
        .iter()
        .filter(|s| s.agent_status == AgentStatus::None || s.agent_status == AgentStatus::Stopped)
        .map(|s| s.name.clone())
        .collect();

    if targets.is_empty() {
        app.set_status("No slots need agents");
        return;
    }

    let total = targets.len();
    app.batch_progress = Some(BatchProgressState::new("Spawn All Agents", total));
    app.mode = Mode::BatchProgress;

    let sm = Arc::clone(slot_manager);
    let tx = event_tx.clone();

    tokio::spawn(async move {
        for (i, name) in targets.iter().enumerate() {
            let _ = tx.send(AppEvent::BatchProgress {
                completed: i,
                total,
                current_slot: Some(name.clone()),
                failure: None,
                done: false,
            });

            if let Err(e) = sm.spawn_agent(name, None, None, None).await {
                let _ = tx.send(AppEvent::BatchProgress {
                    completed: i + 1,
                    total,
                    current_slot: None,
                    failure: Some((name.clone(), e.to_string())),
                    done: false,
                });
            }

            // Brief delay between spawns
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }

        let _ = tx.send(AppEvent::BatchProgress {
            completed: total,
            total,
            current_slot: None,
            failure: None,
            done: true,
        });
    });
}
