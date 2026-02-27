mod common;

use chrono::{TimeZone, Utc};

use ao_core::models::{AgentStatus, SlotStatus};
use ao_tui::app::App;

use common::{make_slot, render_to_string_at};

/// Fixed "now" for deterministic snapshots.
fn fixed_now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap()
}

#[test]
fn detail_panel_with_uptime() {
    let now = fixed_now();
    let mut app = App::new();
    let mut slot = make_slot("auth", SlotStatus::Running, AgentStatus::Active);
    // Aspire started 2h15m ago
    slot.aspire_started_at = Some(now - chrono::Duration::hours(2) - chrono::Duration::minutes(15));
    // Agent started 45m ago
    slot.agent_started_at = Some(now - chrono::Duration::minutes(45));
    // Last output 30s ago
    slot.last_agent_output_at = Some(now - chrono::Duration::seconds(30));
    app.slots.push(slot);
    let output = render_to_string_at(&app, 80, 24, now);
    insta::assert_snapshot!(output);
}

#[test]
fn detail_panel_aspire_only() {
    let now = fixed_now();
    let mut app = App::new();
    let mut slot = make_slot("auth", SlotStatus::Running, AgentStatus::None);
    // Aspire started 10m ago, no agent timestamps
    slot.aspire_started_at = Some(now - chrono::Duration::minutes(10));
    app.slots.push(slot);
    let output = render_to_string_at(&app, 80, 24, now);
    insta::assert_snapshot!(output);
}

#[test]
fn detail_panel_short_uptime() {
    let now = fixed_now();
    let mut app = App::new();
    let mut slot = make_slot("auth", SlotStatus::Starting, AgentStatus::None);
    // Aspire started 5s ago
    slot.aspire_started_at = Some(now - chrono::Duration::seconds(5));
    app.slots.push(slot);
    let output = render_to_string_at(&app, 80, 24, now);
    insta::assert_snapshot!(output);
}
