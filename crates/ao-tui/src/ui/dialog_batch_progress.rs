use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::ui::layout::centered_rect;

/// Render the batch progress overlay dialog.
pub fn render(f: &mut Frame, app: &App) {
    let Some(ref progress) = app.batch_progress else {
        return;
    };

    let area = centered_rect(50, 40, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" {} ", progress.title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // gauge
            Constraint::Length(1), // current slot
            Constraint::Length(1), // blank
            Constraint::Min(3),    // failures
            Constraint::Length(1), // help
        ])
        .split(inner);

    // Progress gauge
    let ratio = if progress.total > 0 {
        progress.completed as f64 / progress.total as f64
    } else {
        0.0
    };
    let gauge = Gauge::default()
        .ratio(ratio)
        .label(format!("{}/{}", progress.completed, progress.total))
        .gauge_style(Style::default().fg(Color::Cyan));
    f.render_widget(gauge, chunks[0]);

    // Current slot
    if let Some(ref current) = progress.current_slot {
        f.render_widget(
            Paragraph::new(Span::styled(
                format!(" Current: {current}"),
                Style::default().fg(Color::White),
            )),
            chunks[1],
        );
    }

    // Failures
    if !progress.failures.is_empty() {
        let mut lines = vec![Line::from(Span::styled(
            " Failed:",
            Style::default().fg(Color::Red),
        ))];
        for (name, err) in &progress.failures {
            lines.push(Line::from(Span::styled(
                format!("  - {name}: {err}"),
                Style::default().fg(Color::Red),
            )));
        }
        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), chunks[3]);
    }

    // Help
    let help_text = if progress.done {
        " Press Esc to dismiss"
    } else {
        " Running..."
    };
    f.render_widget(
        Paragraph::new(Span::styled(
            help_text,
            Style::default().fg(Color::DarkGray),
        )),
        chunks[4],
    );
}
