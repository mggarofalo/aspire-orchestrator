mod common;

use ao_core::models::{AgentStatus, SlotStatus};
use ao_tui::app::{App, Mode, SpawnAgentField};

use common::{make_slot, render_to_string};

#[test]
fn agent_dialog_default() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.mode = Mode::SpawnAgentDialog;
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn agent_dialog_tools_focused() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.mode = Mode::SpawnAgentDialog;
    app.agent_form.focus = SpawnAgentField::AllowedTools;
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn agent_dialog_with_prompt() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.mode = Mode::SpawnAgentDialog;
    app.agent_form.prompt = "Implement dark mode".into();
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn agent_dialog_max_turns_focused() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.mode = Mode::SpawnAgentDialog;
    app.agent_form.focus = SpawnAgentField::MaxTurns;
    app.agent_form.max_turns = "25".into();
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn agent_dialog_all_fields_filled() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.mode = Mode::SpawnAgentDialog;
    app.agent_form.prompt = "Fix the login bug on the auth page".into();
    app.agent_form.allowed_tools = "Bash,Read,Write".into();
    app.agent_form.max_turns = "10".into();
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}
