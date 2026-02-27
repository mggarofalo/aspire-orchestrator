mod common;

use ao_tui::app::{App, CreateSlotField, Mode};

use common::{inject_candidates, render_to_string};

#[test]
fn create_dialog_scanning() {
    let mut app = App::new();
    app.mode = Mode::CreateSlotDialog;
    // scan_loading defaults to true, no candidates yet
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn create_dialog_with_candidates() {
    let mut app = App::new();
    app.mode = Mode::CreateSlotDialog;
    inject_candidates(&mut app);
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn create_dialog_candidate_scrolled() {
    let mut app = App::new();
    app.mode = Mode::CreateSlotDialog;
    inject_candidates(&mut app);
    app.create_form.selected_candidate = Some(2);
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn create_dialog_candidate_at_last() {
    let mut app = App::new();
    app.mode = Mode::CreateSlotDialog;
    inject_candidates(&mut app);
    app.create_form.selected_candidate = Some(4);
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn create_dialog_filtered() {
    let mut app = App::new();
    app.mode = Mode::CreateSlotDialog;
    inject_candidates(&mut app);
    app.create_form.source = "rec".into();
    app.create_form.apply_filter();
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn create_dialog_prompt_focused() {
    let mut app = App::new();
    app.mode = Mode::CreateSlotDialog;
    app.create_form.scan_loading = false;
    app.create_form.focus = CreateSlotField::Prompt;
    // No candidates when prompt is focused
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn create_dialog_prompt_with_text() {
    let mut app = App::new();
    app.mode = Mode::CreateSlotDialog;
    app.create_form.scan_loading = false;
    app.create_form.focus = CreateSlotField::Prompt;
    app.create_form.prompt = "fix the login bug".into();
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}

#[test]
fn create_dialog_source_accepted() {
    let mut app = App::new();
    app.mode = Mode::CreateSlotDialog;
    inject_candidates(&mut app);
    // Accept the first candidate
    app.create_form.accept_selected();
    let output = render_to_string(&app, 80, 24);
    insta::assert_snapshot!(output);
}
