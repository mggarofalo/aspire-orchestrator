use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use ao_core::services::log_tailer::LogSource;

use crate::app::{App, Severity};
use crate::ui::multiplex_filter_bar::SLOT_COLORS;

/// Render the full multiplex log panel (filter bar + log lines).
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // filter bar
            Constraint::Min(5),    // log content
        ])
        .split(area);

    super::multiplex_filter_bar::render(f, chunks[0], app);
    render_log_content(f, chunks[1], app);
}

fn render_log_content(f: &mut Frame, area: Rect, app: &App) {
    let block = Block::default()
        .title(" Multiplexed Log ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    let inner_height = inner.height as usize;

    // Collect filtered entries
    let filtered_indices: Vec<usize> = app
        .log_buffer
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| app.multiplex_filter.matches_entry(entry))
        .map(|(i, _)| i)
        .collect();

    if filtered_indices.is_empty() {
        let empty = Paragraph::new(Span::styled(
            " No log entries matching filters",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        f.render_widget(empty, area);
        return;
    }

    let total = filtered_indices.len();

    // Calculate scroll position
    let scroll = if app.multiplex_auto_follow {
        total.saturating_sub(inner_height)
    } else {
        app.multiplex_scroll.min(total.saturating_sub(inner_height))
    };

    let visible_indices: Vec<usize> = filtered_indices
        .into_iter()
        .skip(scroll)
        .take(inner_height)
        .collect();

    let max_name_len = 12;

    let lines: Vec<Line> = visible_indices
        .iter()
        .map(|&idx| {
            let entry = &app.log_buffer.entries[idx];
            let color = SLOT_COLORS[entry.color_index as usize % 8];

            let source_char = match entry.source {
                LogSource::Agent => "A",
                LogSource::Aspire => "S",
            };

            let name_display = if entry.slot_name.len() > max_name_len {
                format!(
                    "{:>width$}",
                    &entry.slot_name[..max_name_len],
                    width = max_name_len
                )
            } else {
                format!("{:>width$}", entry.slot_name, width = max_name_len)
            };

            let text_color = match entry.severity {
                Severity::Error => Color::Red,
                Severity::Warn => Color::Yellow,
                _ => Color::White,
            };

            let mut spans = vec![
                Span::styled(name_display, Style::default().fg(color)),
                Span::styled(
                    format!(" {source_char} "),
                    Style::default().fg(Color::DarkGray),
                ),
            ];

            // Add text with optional search highlighting
            if let Some(ref re) = app.multiplex_filter.search_regex {
                let text = &entry.text;
                let mut last_end = 0;
                for mat in re.find_iter(text) {
                    if mat.start() > last_end {
                        spans.push(Span::styled(
                            text[last_end..mat.start()].to_string(),
                            Style::default().fg(text_color),
                        ));
                    }
                    spans.push(Span::styled(
                        mat.as_str().to_string(),
                        Style::default().fg(Color::Black).bg(Color::Yellow),
                    ));
                    last_end = mat.end();
                }
                if last_end < text.len() {
                    spans.push(Span::styled(
                        text[last_end..].to_string(),
                        Style::default().fg(text_color),
                    ));
                }
            } else {
                spans.push(Span::styled(
                    entry.text.clone(),
                    Style::default().fg(text_color),
                ));
            }

            Line::from(spans)
        })
        .collect();

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}
