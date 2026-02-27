pub mod dashboard;
pub mod detail_panel;
pub mod dialog_agent;
pub mod dialog_batch_progress;
pub mod dialog_blueprint;
pub mod dialog_confirm;
pub mod dialog_create;
pub mod dialog_help;
pub mod layout;
pub mod log_view;
pub mod multiplex_filter_bar;
pub mod multiplex_log;
pub mod slot_table;
pub mod status_bar;
pub mod terminal_view;

use chrono::{DateTime, Utc};
use ratatui::Frame;

use crate::app::{App, Mode, ViewMode};

/// Top-level render dispatch. Pass `now` to pin timestamps for deterministic output,
/// or `None` to use the current time.
pub fn render(f: &mut Frame, app: &App, now: Option<DateTime<Utc>>) {
    let now = now.unwrap_or_else(Utc::now);
    let chunks = layout::main_layout(f.area());

    // Title bar
    layout::render_title(f, chunks[0]);

    // Main content area depends on mode and view
    match app.mode {
        Mode::Terminal => {
            terminal_view::render(f, chunks[1], app);
        }
        Mode::MultiplexLog => {
            multiplex_log::render(f, chunks[1], app);
        }
        _ => match app.view {
            ViewMode::Dashboard => {
                dashboard::render(f, chunks[1], app);
            }
            ViewMode::SlotList => {
                let content_chunks = layout::content_layout(chunks[1]);
                slot_table::render(f, content_chunks[0], app);
                let right_chunks = layout::right_panel_layout(content_chunks[1]);
                detail_panel::render_with_now(f, right_chunks[0], app, now);

                // Show terminal view if slot has an active agent with terminal data
                let has_terminal = app
                    .selected_slot()
                    .map(|s| app.terminal_parsers.contains_key(&s.name))
                    .unwrap_or(false);

                if has_terminal {
                    terminal_view::render_embedded(f, right_chunks[1], app);
                } else {
                    log_view::render(f, right_chunks[1], app);
                }
            }
        },
    }

    // Status bar / hotkey hints
    status_bar::render(f, chunks[2], app);

    // Overlay dialogs
    match &app.mode {
        Mode::CreateSlotDialog => dialog_create::render(f, app),
        Mode::SpawnAgentDialog => dialog_agent::render(f, app),
        Mode::ConfirmDialog { message, .. } => dialog_confirm::render(f, message),
        Mode::HelpDialog => dialog_help::render(f),
        Mode::Loading(msg) => dialog_confirm::render_loading(f, msg),
        Mode::BlueprintListDialog => dialog_blueprint::render_list(f, app),
        Mode::BlueprintSaveDialog => dialog_blueprint::render_save(f, app),
        Mode::BatchProgress => dialog_batch_progress::render(f, app),
        Mode::SlotList | Mode::MultiplexLog | Mode::Terminal => {}
    }
}
