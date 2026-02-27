use chrono::{DateTime, Utc};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use ao_core::models::{AgentStatus, Slot, SlotStatus};

use crate::app::App;

pub fn render_with_now(f: &mut Frame, area: Rect, app: &App, now: DateTime<Utc>) {
    let block = Block::default()
        .title(" Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let Some(slot) = app.selected_slot() else {
        let empty = Paragraph::new(" No slot selected").block(block);
        f.render_widget(empty, area);
        return;
    };

    let lines = build_detail_lines(slot, now);
    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn build_detail_lines(slot: &Slot, now: DateTime<Utc>) -> Vec<Line<'static>> {
    let mut lines = vec![
        detail_line("Branch", &slot.branch, Color::White),
        detail_line(
            "Status",
            &format!("{:?}", slot.status),
            status_color(&slot.status),
        ),
        detail_line(
            "Agent",
            &format!("{:?}", slot.agent_status),
            agent_color(&slot.agent_status),
        ),
    ];

    if let Some(started) = slot.aspire_started_at {
        let d = now.signed_duration_since(started);
        lines.push(detail_line("Uptime", &format_duration(d), Color::Green));
    }

    if let Some(started) = slot.agent_started_at {
        let d = now.signed_duration_since(started);
        lines.push(detail_line("Agent Up", &format_duration(d), Color::Cyan));
    }

    if let Some(last_output) = slot.last_agent_output_at {
        let d = now.signed_duration_since(last_output);
        lines.push(detail_line(
            "Idle",
            &format!("{} ago", format_duration(d)),
            Color::DarkGray,
        ));
    }

    if let Some(ref url) = slot.services.dashboard_url {
        lines.push(detail_line("Dashboard", url, Color::Cyan));
    }

    if !slot.services.service_urls.is_empty() {
        let urls: Vec<String> = slot
            .services
            .service_urls
            .iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect();
        for url_line in &urls {
            lines.push(detail_line("Service", url_line, Color::Cyan));
        }
    }

    if !slot.port_allocations.is_empty() {
        let ports: String = slot
            .port_allocations
            .iter()
            .map(|p| format!("{}={}", p.name, p.port))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(detail_line("Ports", &ports, Color::White));
    }

    lines
}

fn format_duration(d: chrono::Duration) -> String {
    let total_secs = d.num_seconds().max(0);
    let hours = total_secs / 3600;
    let mins = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{hours}h {mins:02}m")
    } else if mins > 0 {
        format!("{mins}m {secs:02}s")
    } else {
        format!("{secs}s")
    }
}

fn detail_line(label: &str, value: &str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {label:<10} "),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(value.to_string(), Style::default().fg(color)),
    ])
}

fn status_color(status: &SlotStatus) -> Color {
    match status {
        SlotStatus::Running => Color::Green,
        SlotStatus::Starting | SlotStatus::Stopping => Color::Yellow,
        SlotStatus::Ready => Color::White,
        SlotStatus::Error => Color::Red,
        SlotStatus::Provisioning => Color::DarkGray,
    }
}

fn agent_color(status: &AgentStatus) -> Color {
    match status {
        AgentStatus::Active => Color::Cyan,
        AgentStatus::Starting => Color::Yellow,
        AgentStatus::Blocked => Color::Red,
        AgentStatus::Stopped | AgentStatus::None => Color::DarkGray,
    }
}
