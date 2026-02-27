use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

use ao_core::models::{AgentStatus, Slot, SlotStatus};

use crate::app::App;

pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .slots
        .iter()
        .map(|slot| {
            let aspire_icon = aspire_status_icon(slot);
            let agent_icon = agent_status_icon(slot);

            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(&slot.name, Style::default().fg(Color::White)),
                Span::raw("  "),
                aspire_icon,
                Span::raw(" "),
                agent_icon,
                Span::raw("  "),
                Span::styled(
                    truncate_branch(&slot.branch, 20),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Slots ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .highlight_style(
            Style::default()
                .bg(Color::Rgb(0x1A, 0x3A, 0x5C))
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

    let mut state = ListState::default();
    if !app.slots.is_empty() {
        state.select(Some(app.selected_index));
    }

    f.render_stateful_widget(list, area, &mut state);
}

fn aspire_status_icon(slot: &Slot) -> Span<'static> {
    match slot.status {
        SlotStatus::Running => Span::styled("▶", Style::default().fg(Color::Green)),
        SlotStatus::Starting => Span::styled("◎", Style::default().fg(Color::Yellow)),
        SlotStatus::Stopping => Span::styled("◉", Style::default().fg(Color::Yellow)),
        SlotStatus::Ready => Span::styled("■", Style::default().fg(Color::DarkGray)),
        SlotStatus::Error => Span::styled("✗", Style::default().fg(Color::Red)),
        SlotStatus::Provisioning => Span::styled("…", Style::default().fg(Color::DarkGray)),
    }
}

fn agent_status_icon(slot: &Slot) -> Span<'static> {
    match slot.agent_status {
        AgentStatus::Active => Span::styled("●", Style::default().fg(Color::Cyan)),
        AgentStatus::Starting => Span::styled("◎", Style::default().fg(Color::Yellow)),
        AgentStatus::Blocked => Span::styled("⊘", Style::default().fg(Color::Red)),
        AgentStatus::Stopped => Span::styled("○", Style::default().fg(Color::DarkGray)),
        AgentStatus::None => Span::styled("○", Style::default().fg(Color::DarkGray)),
    }
}

fn truncate_branch(branch: &str, max_len: usize) -> String {
    if branch.chars().count() <= max_len {
        branch.to_string()
    } else {
        let truncated: String = branch.chars().take(max_len - 3).collect();
        format!("{truncated}...")
    }
}
