mod common;

use ao_core::models::{AgentStatus, SlotStatus};
use ao_tui::app::{App, Mode};

use common::{make_slot, render_to_string};

#[test]
fn help_dialog_renders() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.mode = Mode::HelpDialog;
    let output = render_to_string(&app, 80, 30);
    insta::assert_snapshot!(output);
}

#[test]
fn help_dialog_wide_terminal() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.mode = Mode::HelpDialog;
    let output = render_to_string(&app, 120, 40);
    insta::assert_snapshot!(output);
}

#[test]
fn help_dialog_narrow_terminal() {
    let mut app = App::new();
    app.slots
        .push(make_slot("auth", SlotStatus::Ready, AgentStatus::None));
    app.mode = Mode::HelpDialog;
    let output = render_to_string(&app, 60, 24);
    insta::assert_snapshot!(output);
}
