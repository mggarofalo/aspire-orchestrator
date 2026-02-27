use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::{App, BlueprintSaveField};
use crate::ui::layout::centered_rect;

/// Render the blueprint list dialog (load/delete blueprints).
pub fn render_list(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 50, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Blueprints ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Min(3),    // list
            Constraint::Length(1), // help
        ])
        .split(inner);

    if app.blueprint_list.loading {
        f.render_widget(
            Paragraph::new(Span::styled(
                " Loading blueprints...",
                Style::default().fg(Color::DarkGray),
            )),
            chunks[0],
        );
    } else if app.blueprint_list.names.is_empty() {
        f.render_widget(
            Paragraph::new(Span::styled(
                " No blueprints saved. Press [S] to save current slots.",
                Style::default().fg(Color::DarkGray),
            )),
            chunks[0],
        );
    } else {
        let items: Vec<ListItem> = app
            .blueprint_list
            .names
            .iter()
            .map(|name| {
                ListItem::new(Line::from(Span::styled(
                    format!("  {name}"),
                    Style::default().fg(Color::White),
                )))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(0x1A, 0x3A, 0x5C))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        let mut state = ListState::default();
        if !app.blueprint_list.names.is_empty() {
            state.select(Some(app.blueprint_list.selected));
        }
        f.render_stateful_widget(list, chunks[0], &mut state);
    }

    // Help text
    let help = Line::from(vec![
        Span::styled(" Enter", Style::default().fg(Color::Yellow)),
        Span::styled(" load  ", Style::default().fg(Color::DarkGray)),
        Span::styled("S", Style::default().fg(Color::Yellow)),
        Span::styled(" save  ", Style::default().fg(Color::DarkGray)),
        Span::styled("D", Style::default().fg(Color::Yellow)),
        Span::styled(" delete  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" close", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(help), chunks[1]);
}

/// Render the blueprint save dialog.
pub fn render_save(f: &mut Frame, app: &App) {
    let area = centered_rect(50, 35, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Save Blueprint ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // name label
            Constraint::Length(1), // name input
            Constraint::Length(1), // blank
            Constraint::Length(1), // desc label
            Constraint::Length(1), // desc input
            Constraint::Length(1), // blank
            Constraint::Length(1), // help
        ])
        .split(inner);

    let name_style = field_style(app.blueprint_save.focus == BlueprintSaveField::Name);
    let desc_style = field_style(app.blueprint_save.focus == BlueprintSaveField::Description);

    f.render_widget(
        Paragraph::new(Span::styled("Name:", Style::default().fg(Color::White))),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", app.blueprint_save.name),
            name_style,
        ))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(name_style),
        ),
        chunks[1],
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            "Description (optional):",
            Style::default().fg(Color::White),
        )),
        chunks[3],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", app.blueprint_save.description),
            desc_style,
        ))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(desc_style),
        ),
        chunks[4],
    );

    let help = Line::from(vec![
        Span::styled(" Tab", Style::default().fg(Color::Yellow)),
        Span::styled(" switch  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::styled(" save  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(help), chunks[6]);
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
