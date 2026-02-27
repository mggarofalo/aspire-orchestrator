use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, CreateSlotField};
use crate::ui::layout::centered_rect;

pub fn render(f: &mut Frame, app: &App) {
    let area = centered_rect(60, 55, f.area());

    // Clear the background
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" New Slot ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Determine how many candidate rows to show (0 to MAX_VISIBLE)
    const MAX_VISIBLE: usize = 6;
    let show_candidates = app.create_form.focus == CreateSlotField::Source
        && (!app.create_form.filtered_candidates.is_empty()
            || (app.create_form.scan_loading && app.create_form.all_candidates.is_empty()));
    let candidate_rows = if show_candidates {
        if app.create_form.scan_loading && app.create_form.all_candidates.is_empty() {
            1 // "Scanning..." row
        } else {
            app.create_form.filtered_candidates.len().min(MAX_VISIBLE) as u16
        }
    } else {
        0
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),              // source label
            Constraint::Length(1),              // source input
            Constraint::Length(candidate_rows), // candidate list (dynamic)
            Constraint::Length(1),              // blank
            Constraint::Length(1),              // prompt label
            Constraint::Min(3),                 // prompt input
            Constraint::Length(1),              // blank
            Constraint::Length(1),              // help text
        ])
        .split(inner);

    let source_style = field_style(app.create_form.focus == CreateSlotField::Source);
    let prompt_style = field_style(app.create_form.focus == CreateSlotField::Prompt);

    // Source label
    f.render_widget(
        Paragraph::new(Span::styled(
            "Source Repository:",
            Style::default().fg(Color::White),
        )),
        chunks[0],
    );

    // Source input — render text directly (no Block border, which would consume the row)
    let source_display = format!(" {}", app.create_form.source);
    let underline_style = source_style.add_modifier(Modifier::UNDERLINED);
    f.render_widget(
        Paragraph::new(Span::styled(source_display, underline_style)),
        chunks[1],
    );

    // Candidate list area
    if candidate_rows > 0 {
        if app.create_form.scan_loading && app.create_form.all_candidates.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " Scanning...",
                    Style::default().fg(Color::DarkGray),
                )),
                chunks[2],
            );
        } else {
            let items: Vec<ListItem> = app
                .create_form
                .filtered_candidates
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let is_selected = app.create_form.selected_candidate == Some(i);
                    let prefix = if is_selected { " > " } else { "   " };
                    let hint = c.location_hint();

                    let line = Line::from(vec![
                        Span::styled(
                            prefix,
                            if is_selected {
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::White)
                            },
                        ),
                        Span::styled(
                            c.display_label(),
                            if is_selected {
                                Style::default()
                                    .fg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(Color::White)
                            },
                        ),
                        Span::styled("  ", Style::default()),
                        Span::styled(hint, Style::default().fg(Color::DarkGray)),
                    ]);

                    let style = if is_selected {
                        Style::default().bg(Color::DarkGray)
                    } else {
                        Style::default()
                    };

                    ListItem::new(line).style(style)
                })
                .collect();

            let list = List::new(items)
                .highlight_style(Style::default()) // styling handled per-item above
                .highlight_symbol("");

            let mut list_state =
                ListState::default().with_selected(app.create_form.selected_candidate);
            f.render_stateful_widget(list, chunks[2], &mut list_state);
        }
    }

    // Prompt label
    f.render_widget(
        Paragraph::new(Span::styled(
            "Prompt (optional):",
            Style::default().fg(Color::White),
        )),
        chunks[4],
    );

    // Prompt input
    let prompt_text = format!(" {}", app.create_form.prompt);
    f.render_widget(
        Paragraph::new(Span::styled(prompt_text, prompt_style)).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(prompt_style),
        ),
        chunks[5],
    );

    // Help text
    let help = if show_candidates && !app.create_form.filtered_candidates.is_empty() {
        Line::from(vec![
            Span::styled(" Up/Down", Style::default().fg(Color::Yellow)),
            Span::styled(" navigate  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::styled(" select  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled(" Tab", Style::default().fg(Color::Yellow)),
            Span::styled(" switch field  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::styled(" create  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
        ])
    };
    f.render_widget(Paragraph::new(help), chunks[7]);
}

fn field_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    }
}
