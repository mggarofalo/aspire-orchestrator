use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Main vertical layout: [title bar] [content] [status bar]
pub fn main_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title
            Constraint::Min(10),   // content
            Constraint::Length(2), // status bar
        ])
        .split(area)
        .to_vec()
}

/// Content area: [left: slot table] [right: detail+log]
pub fn content_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35), // slot table
            Constraint::Percentage(65), // detail + log
        ])
        .split(area)
        .to_vec()
}

/// Right panel: [detail info] [log output]
pub fn right_panel_layout(area: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // detail panel
            Constraint::Min(5),    // log view
        ])
        .split(area)
        .to_vec()
}

/// Render the title bar.
pub fn render_title(f: &mut Frame, area: Rect) {
    let title = Paragraph::new(Line::from(vec![
        Span::styled(" AspireOrchestrator ", Style::default().fg(Color::Yellow)),
        Span::styled("(Rust)", Style::default().fg(Color::DarkGray)),
    ]));
    f.render_widget(title, area);
}

/// Create a centered popup area of a given percentage of the screen.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
