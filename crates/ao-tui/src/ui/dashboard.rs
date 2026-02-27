use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Sparkline};
use ratatui::Frame;

use ao_core::models::{AgentStatus, SlotStatus};

use crate::app::App;

/// Render the full dashboard view with slot cards in a grid.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(5)])
        .split(area);

    render_summary_header(f, chunks[0], app);

    if app.slots.is_empty() {
        let msg = Paragraph::new(Line::from(Span::styled(
            "  No slots. Press [N] to create one.",
            Style::default().fg(Color::DarkGray),
        )));
        f.render_widget(msg, chunks[1]);
        return;
    }

    render_card_grid(f, chunks[1], app);
}

fn render_summary_header(f: &mut Frame, area: Rect, app: &App) {
    let running = app
        .slots
        .iter()
        .filter(|s| s.status == SlotStatus::Running)
        .count();
    let agents = app
        .slots
        .iter()
        .filter(|s| s.agent_status == AgentStatus::Active)
        .count();
    let blocked = app
        .slots
        .iter()
        .filter(|s| s.agent_status == AgentStatus::Blocked)
        .count();
    let errors = app
        .slots
        .iter()
        .filter(|s| s.status == SlotStatus::Error)
        .count();

    let line = Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(
            format!("{running} running"),
            Style::default().fg(Color::Green),
        ),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{agents} agents"), Style::default().fg(Color::Cyan)),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{blocked} blocked"),
            if blocked > 0 {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
        Span::styled(" | ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{errors} errors"),
            if errors > 0 {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

fn render_card_grid(f: &mut Frame, area: Rect, app: &App) {
    let cols = app.dashboard_columns();
    let rows = app.slots.len().div_ceil(cols);

    // Split into rows
    let row_constraints: Vec<Constraint> = (0..rows)
        .map(|_| Constraint::Min(9))
        .chain(std::iter::once(Constraint::Length(0)))
        .collect();

    let row_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(row_constraints)
        .split(area);

    for row in 0..rows {
        let slots_in_row = if row == rows - 1 {
            app.slots.len() - row * cols
        } else {
            cols
        };

        let col_constraints: Vec<Constraint> = (0..cols)
            .map(|c| {
                if c < slots_in_row {
                    Constraint::Percentage((100 / cols) as u16)
                } else {
                    Constraint::Min(0)
                }
            })
            .collect();

        let col_areas = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(col_constraints)
            .split(row_areas[row]);

        for col in 0..slots_in_row {
            let idx = row * cols + col;
            if idx < app.slots.len() {
                render_card(f, col_areas[col], app, idx);
            }
        }
    }
}

fn render_card(f: &mut Frame, area: Rect, app: &App, idx: usize) {
    let slot = &app.slots[idx];
    let is_selected = idx == app.dashboard_selected;
    let activity = app.activity.get(&slot.name);
    let needs_attention = activity.is_some_and(|a| a.needs_attention);

    let border_color = if needs_attention {
        Color::Red
    } else if is_selected {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let title = if needs_attention {
        format!(" {} [!] ", slot.name)
    } else {
        format!(" {} ", slot.name)
    };

    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if inner.height < 2 {
        return;
    }

    // Build card content lines
    let mut lines: Vec<Line> = Vec::new();

    // Branch
    let branch_display = truncate(&slot.branch, inner.width as usize - 2);
    lines.push(Line::from(Span::styled(
        format!(" {branch_display}"),
        Style::default().fg(Color::DarkGray),
    )));

    // Aspire status
    let (aspire_icon, aspire_color) = aspire_display(&slot.status);
    lines.push(Line::from(vec![
        Span::styled(" Aspire: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{aspire_icon} {:?}", slot.status),
            Style::default().fg(aspire_color),
        ),
    ]));

    // Agent status
    let (agent_icon, agent_color) = agent_display(&slot.agent_status);
    lines.push(Line::from(vec![
        Span::styled(" Agent:  ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{agent_icon} {:?}", slot.agent_status),
            Style::default().fg(agent_color),
        ),
    ]));

    // Idle time
    if let Some(act) = activity {
        if let Some(last_ts) = act.log_timestamps.back() {
            let idle_secs = std::time::Instant::now().duration_since(*last_ts).as_secs();
            let idle_str = if idle_secs >= 3600 {
                format!("{}h {}m", idle_secs / 3600, (idle_secs % 3600) / 60)
            } else if idle_secs >= 60 {
                format!("{}m {}s", idle_secs / 60, idle_secs % 60)
            } else {
                format!("{idle_secs}s")
            };
            lines.push(Line::from(vec![
                Span::styled(" Idle: ", Style::default().fg(Color::DarkGray)),
                Span::styled(idle_str, Style::default().fg(Color::White)),
            ]));
        }
    }

    // Dashboard URL (port only)
    if let Some(ref url) = slot.services.dashboard_url {
        if let Some(port) = extract_port(url) {
            lines.push(Line::from(vec![
                Span::styled(" Dash: ", Style::default().fg(Color::DarkGray)),
                Span::styled(format!(":{port}"), Style::default().fg(Color::Cyan)),
            ]));
        }
    }

    // Last log line
    if let Some(act) = activity {
        if let Some(ref last_line) = act.last_log_line {
            lines.push(Line::from(Span::styled(
                format!(" > {}", truncate(last_line, inner.width as usize - 4)),
                Style::default().fg(Color::DarkGray),
            )));
        }
    }

    // Render text lines
    let text_height = inner.height.saturating_sub(2); // leave room for sparkline
    let text_lines: Vec<Line> = lines.into_iter().take(text_height as usize).collect();

    let text_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: text_height,
    };
    f.render_widget(Paragraph::new(text_lines), text_area);

    // Sparkline at bottom of card
    if inner.height >= 3 {
        let sparkline_area = Rect {
            x: inner.x + 1,
            y: inner.y + inner.height - 2,
            width: inner.width.saturating_sub(2).min(20),
            height: 1,
        };
        if let Some(act) = activity {
            let sparkline = Sparkline::default()
                .data(&act.sparkline_data)
                .max(8)
                .style(Style::default().fg(Color::Cyan));
            f.render_widget(sparkline, sparkline_area);
        }
    }
}

fn aspire_display(status: &SlotStatus) -> (&'static str, Color) {
    match status {
        SlotStatus::Running => (">", Color::Green),
        SlotStatus::Starting => ("~", Color::Yellow),
        SlotStatus::Stopping => ("~", Color::Yellow),
        SlotStatus::Ready => ("-", Color::DarkGray),
        SlotStatus::Error => ("!", Color::Red),
        SlotStatus::Provisioning => (".", Color::DarkGray),
    }
}

fn agent_display(status: &AgentStatus) -> (&'static str, Color) {
    match status {
        AgentStatus::Active => ("*", Color::Cyan),
        AgentStatus::Starting => ("~", Color::Yellow),
        AgentStatus::Blocked => ("!", Color::Red),
        AgentStatus::Stopped | AgentStatus::None => ("o", Color::DarkGray),
    }
}

fn truncate(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else if max > 3 {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{truncated}...")
    } else {
        s.chars().take(max).collect()
    }
}

fn extract_port(url: &str) -> Option<u16> {
    url.rsplit(':').next()?.split('/').next()?.parse().ok()
}
