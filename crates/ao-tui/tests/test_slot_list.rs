mod common;

use ao_core::models::{AgentStatus, PortAllocation, SlotStatus};
use ao_tui::app::{App, LogSource};

use common::{make_slot, render_to_string};

#[test]
fn empty_slot_list() {
    let app = App::new();
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn single_slot_ready() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn multiple_slots_mixed_status() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.slots.push(make_slot(
        "payments",
        SlotStatus::Running,
        AgentStatus::Active,
    ));
    app.slots
        .push(make_slot("search", SlotStatus::Error, AgentStatus::Stopped));
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn slot_selected_second() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.slots.push(make_slot(
        "payments",
        SlotStatus::Running,
        AgentStatus::Active,
    ));
    app.slots
        .push(make_slot("search", SlotStatus::Error, AgentStatus::Stopped));
    app.selected_index = 1;
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn status_message_shown() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.set_status("Aspire started");
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn error_status_message() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.set_status("Error: connection refused");
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn error_failed_status_message() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.set_status("Start failed: port in use");
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn all_aspire_status_icons() {
    let mut app = App::new();
    app.slots.push(make_slot(
        "slot-a",
        SlotStatus::Provisioning,
        AgentStatus::None,
    ));
    app.slots.push(make_slot(
        "slot-b",
        SlotStatus::Starting,
        AgentStatus::Starting,
    ));
    app.slots.push(make_slot(
        "slot-c",
        SlotStatus::Stopping,
        AgentStatus::Blocked,
    ));
    app.slots.push(make_slot(
        "slot-d",
        SlotStatus::Running,
        AgentStatus::Active,
    ));
    app.slots
        .push(make_slot("slot-e", SlotStatus::Ready, AgentStatus::Stopped));
    app.slots
        .push(make_slot("slot-f", SlotStatus::Error, AgentStatus::None));
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn agent_blocked_status() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Running, AgentStatus::Blocked));
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn long_branch_truncation() {
    let mut app = App::new();
    let mut slot = make_slot("auth", SlotStatus::Ready, AgentStatus::None);
    slot.branch = "feature/very-long-branch-name-that-exceeds-limit".into();
    app.slots.push(slot);
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn aspire_log_source_toggle() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.log_source = LogSource::Aspire;
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn detail_panel_with_services() {
    let mut app = App::new();
    let mut slot = make_slot("auth", SlotStatus::Running, AgentStatus::Active);
    slot.services.dashboard_url = Some("http://localhost:15000".into());
    slot.services
        .service_urls
        .insert("api".into(), "http://localhost:5001".into());
    slot.port_allocations = vec![
        PortAllocation {
            name: "http".into(),
            port: 5001,
        },
        PortAllocation {
            name: "https".into(),
            port: 5002,
        },
    ];
    app.slots.push(slot);
    let output = render_to_string(&app, 100, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn log_view_with_content() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Running, AgentStatus::Active));
    app.log_lines = vec![
        "2025-06-15 10:00:01 Starting agent...".into(),
        "2025-06-15 10:00:02 Loading configuration".into(),
        "2025-06-15 10:00:03 Connected to workspace".into(),
        "2025-06-15 10:00:04 Running task: implement auth".into(),
        "2025-06-15 10:00:05 Task completed successfully".into(),
    ];
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn log_view_aspire_with_content() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Running, AgentStatus::Active));
    app.log_source = LogSource::Aspire;
    app.log_lines = vec![
        "info: Aspire dashboard running at http://localhost:15000".into(),
        "info: Service 'api' started on port 5001".into(),
        "info: Service 'web' started on port 5002".into(),
    ];
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}
