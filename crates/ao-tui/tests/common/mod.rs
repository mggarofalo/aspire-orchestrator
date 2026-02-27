// Each test binary compiles this module independently and uses a different
// subset of helpers, so unused-function warnings are expected.
#![allow(dead_code)]

use chrono::{DateTime, Utc};

use ao_core::models::{AgentStatus, RepoCandidate, Slot, SlotStatus};
use ao_tui::app::App;
use ao_tui::ui;
use ratatui::{backend::TestBackend, Terminal};

/// Render the app to a string using a TestBackend of the given dimensions.
pub fn render_to_string(app: &App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| ui::render(f, app, None)).unwrap();
    terminal.backend().to_string()
}

/// Render the app with a fixed `now` for deterministic timestamp snapshots.
pub fn render_to_string_at(app: &App, width: u16, height: u16, now: DateTime<Utc>) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|f| ui::render(f, app, Some(now))).unwrap();
    terminal.backend().to_string()
}

/// Build a Slot with the given name, status, and agent status.
pub fn make_slot(name: &str, status: SlotStatus, agent_status: AgentStatus) -> Slot {
    let mut slot = Slot::new(
        name.to_string(),
        format!("C:/Users/test/Source/{name}"),
        format!("feature/{name}"),
        format!("C:/slots/{name}"),
    );
    slot.status = status;
    slot.agent_status = agent_status;
    slot
}

/// Return 5 hardcoded repo candidates (same as headless mode).
pub fn make_candidates() -> Vec<RepoCandidate> {
    vec![
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
    ]
}

/// Inject candidates into the create form: set all_candidates, clear scan_loading,
/// and apply the filter to populate filtered_candidates.
pub fn inject_candidates(app: &mut App) {
    app.create_form.all_candidates = make_candidates();
    app.create_form.scan_loading = false;
    app.create_form.apply_filter();
}
