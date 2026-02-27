use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Mode, ViewMode};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    // First line: status message or blank
    let status_line = if let Some(ref msg) = app.status_message {
        let color = if msg.starts_with("Error") || msg.contains("failed") {
            Color::Red
        } else {
            Color::Green
        };
        Line::from(Span::styled(format!(" {msg}"), Style::default().fg(color)))
    } else {
        Line::from("")
    };

    // Second line: context-sensitive hotkey hints
    let hints = match (&app.mode, &app.view) {
        (Mode::Terminal, _) => Line::from(vec![
            hint("Esc", "back"),
            Span::raw("  All other keys forwarded to agent terminal"),
        ]),
        (Mode::MultiplexLog, _) => Line::from(vec![
            hint("1-6", "toggle"),
            Span::raw(" "),
            hint("Tab", "src"),
            Span::raw(" "),
            hint("/", "search"),
            Span::raw(" "),
            hint("E/W", "err"),
            Span::raw(" "),
            hint("F", "follow"),
            Span::raw(" "),
            hint("M", "back"),
        ]),
        (_, ViewMode::Dashboard) => Line::from(vec![
            hint("Tab", "list"),
            Span::raw(" "),
            hint("Arrows", "nav"),
            Span::raw(" "),
            hint("Enter", "select"),
            Span::raw(" "),
            hint("N", "ew"),
            Span::raw(" "),
            hint("M", "ultiplex"),
            Span::raw(" "),
            hint("B", "lueprints"),
            Span::raw(" "),
            hint("?", "help"),
            Span::raw(" "),
            hint("Q", "uit"),
        ]),
        _ => Line::from(vec![
            hint("N", "ew"),
            Span::raw(" "),
            hint("S", "tart"),
            Span::raw(" "),
            hint("K", "ill"),
            Span::raw(" "),
            hint("D", "estroy"),
            Span::raw(" "),
            hint("A", "gent"),
            Span::raw(" "),
            hint("R", "ebase"),
            Span::raw(" "),
            hint("G", "push"),
            Span::raw(" "),
            hint("P", "term"),
            Span::raw(" "),
            hint("L", "og"),
            Span::raw(" "),
            hint("M", "ultiplex"),
            Span::raw(" "),
            hint("Tab", "dash"),
            Span::raw(" "),
            hint("B", "lueprints"),
            Span::raw(" "),
            hint("?", "help"),
            Span::raw(" "),
            hint("Q", "uit"),
        ]),
    };

    let widget = Paragraph::new(vec![status_line, hints]);
    f.render_widget(widget, area);
}

fn hint(key: &str, label: &str) -> Span<'static> {
    Span::styled(
        format!("[{key}]{label}"),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
}
