use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{App, SpawnAgentField};
use crate::ui::layout::centered_rect;

pub fn render(f: &mut Frame, app: &App) {
    let slot_name = app
        .selected_slot()
        .map(|s| s.name.clone())
        .unwrap_or_default();

    let area = centered_rect(70, 50, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(format!(" Spawn Agent - {slot_name} "))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1), // prompt label
            Constraint::Min(3),    // prompt input
            Constraint::Length(1), // blank
            Constraint::Length(1), // tools label
            Constraint::Length(1), // tools input
            Constraint::Length(1), // blank
            Constraint::Length(1), // max turns label
            Constraint::Length(1), // max turns input
            Constraint::Length(1), // blank
            Constraint::Length(1), // help text
        ])
        .split(inner);

    let prompt_style = field_style(app.agent_form.focus == SpawnAgentField::Prompt);
    let tools_style = field_style(app.agent_form.focus == SpawnAgentField::AllowedTools);
    let turns_style = field_style(app.agent_form.focus == SpawnAgentField::MaxTurns);

    // Prompt
    f.render_widget(
        Paragraph::new(Span::styled("Prompt:", Style::default().fg(Color::White))),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", app.agent_form.prompt),
            prompt_style,
        ))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(prompt_style),
        ),
        chunks[1],
    );

    // Allowed tools
    f.render_widget(
        Paragraph::new(Span::styled(
            "Allowed Tools:",
            Style::default().fg(Color::White),
        )),
        chunks[3],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", app.agent_form.allowed_tools),
            tools_style,
        ))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(tools_style),
        ),
        chunks[4],
    );

    // Max turns
    f.render_widget(
        Paragraph::new(Span::styled(
            "Max Turns (optional):",
            Style::default().fg(Color::White),
        )),
        chunks[6],
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {}", app.agent_form.max_turns),
            turns_style,
        ))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(turns_style),
        ),
        chunks[7],
    );

    // Help
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(" Tab", Style::default().fg(Color::Cyan)),
            Span::styled(" switch  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Enter", Style::default().fg(Color::Cyan)),
            Span::styled(" spawn  ", Style::default().fg(Color::DarkGray)),
            Span::styled("Esc", Style::default().fg(Color::Cyan)),
            Span::styled(" cancel", Style::default().fg(Color::DarkGray)),
        ])),
        chunks[9],
    );
}

fn field_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    }
}
