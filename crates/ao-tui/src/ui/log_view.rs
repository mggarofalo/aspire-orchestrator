use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, LogSource};

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let source_label = match app.log_source {
        LogSource::Agent => "Agent Log",
        LogSource::Aspire => "Aspire Log",
    };

    let block = Block::default()
        .title(format!(" {source_label} [L] toggle "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if app.log_lines.is_empty() {
        let empty = Paragraph::new(Span::styled(
            " No log output",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        f.render_widget(empty, area);
        return;
    }

    let inner_height = area.height.saturating_sub(2) as usize; // borders
    let total_lines = app.log_lines.len();

    // Calculate scroll offset for auto-follow
    let scroll = if app.log_auto_follow {
        total_lines.saturating_sub(inner_height)
    } else {
        app.log_scroll
    };

    let lines: Vec<Line> = app
        .log_lines
        .iter()
        .skip(scroll)
        .take(inner_height)
        .map(|l| Line::from(Span::styled(l.clone(), Style::default().fg(Color::White))))
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}
