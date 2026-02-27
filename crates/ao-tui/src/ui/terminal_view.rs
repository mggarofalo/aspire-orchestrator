use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders};
use ratatui::Frame;
use tui_term::widget::PseudoTerminal;

use crate::app::App;

/// Render an embedded terminal view for the selected slot's agent.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let slot_name = app
        .selected_slot()
        .map(|s| s.name.clone())
        .unwrap_or_default();

    let block = Block::default()
        .title(format!(" Terminal ({slot_name}) [Esc] back "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    if let Some(parser) = app.terminal_parsers.get(&slot_name) {
        let terminal = PseudoTerminal::new(parser.screen()).block(block);
        f.render_widget(terminal, area);
    } else {
        let block = block.title(format!(" Terminal ({slot_name}) — no agent running "));
        f.render_widget(block, area);
    }
}

/// Render the terminal widget in the log panel area (embedded, not full-screen).
pub fn render_embedded(f: &mut Frame, area: Rect, app: &App) {
    let slot_name = app
        .selected_slot()
        .map(|s| s.name.clone())
        .unwrap_or_default();

    let block = Block::default()
        .title(" Terminal (agent) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if let Some(parser) = app.terminal_parsers.get(&slot_name) {
        let terminal = PseudoTerminal::new(parser.screen()).block(block);
        f.render_widget(terminal, area);
    } else {
        f.render_widget(block, area);
    }
}
