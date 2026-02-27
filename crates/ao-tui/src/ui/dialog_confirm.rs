use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::ui::layout::centered_rect;

pub fn render(f: &mut Frame, message: &str) {
    let area = centered_rect(50, 20, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Confirm ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([Constraint::Min(2), Constraint::Length(1)])
        .split(inner);

    // Message
    f.render_widget(
        Paragraph::new(Span::styled(
            message.to_string(),
            Style::default().fg(Color::White),
        ))
        .wrap(Wrap { trim: false }),
        chunks[0],
    );

    // Buttons hint
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" [Y]", Style::default().fg(Color::Green)),
            Span::styled("es  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[N]", Style::default().fg(Color::Red)),
            Span::styled("o / Esc", Style::default().fg(Color::DarkGray)),
        ])),
        chunks[1],
    );
}

pub fn render_loading(f: &mut Frame, message: &str) {
    let area = centered_rect(40, 15, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let inner = block.inner(area);
    f.render_widget(block, area);

    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {message}"),
            Style::default().fg(Color::Yellow),
        )),
        inner,
    );
}
