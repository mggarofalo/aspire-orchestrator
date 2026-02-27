mod common;

use ao_tui::app::{App, ConfirmAction, Mode};

use common::render_to_string;

#[test]
fn confirm_destroy() {
    let mut app = App::new();
    app.mode = Mode::ConfirmDialog {
        message: "Destroy slot 'auth'? This will delete the clone directory.".into(),
        action: ConfirmAction::DestroySlot("auth".into()),
    };
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn confirm_quit() {
    let mut app = App::new();
    app.mode = Mode::ConfirmDialog {
        message: "Quit orchestrator? Running Aspire stacks will be stopped.".into(),
        action: ConfirmAction::Quit,
    };
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn loading_creating() {
    let mut app = App::new();
    app.mode = Mode::Loading("Creating slot feature-x...".into());
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}
