use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::ui::layout::centered_rect;

pub fn render(f: &mut Frame) {
    let area = centered_rect(65, 80, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" Help — Keybindings ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let lines = vec![
        section_header("Slot List"),
        key_line("N", "Create new slot"),
        key_line("S", "Start Aspire stack"),
        key_line("K", "Kill (stop) Aspire stack"),
        key_line("D", "Destroy slot"),
        key_line("A", "Spawn agent"),
        key_line("R", "Rebase branch"),
        key_line("G", "Git push"),
        key_line("P", "Open terminal for agent"),
        key_line("L", "Toggle log source"),
        key_line("M", "Open multiplexed log"),
        key_line("B", "Open blueprints"),
        key_line("Tab", "Toggle dashboard view"),
        key_line("?", "Show this help"),
        key_line("Q", "Quit orchestrator"),
        Line::from(""),
        section_header("Batch Operations"),
        key_line("Shift+S", "Start all Aspire"),
        key_line("Shift+K", "Stop all Aspire"),
        key_line("Shift+R", "Rebase all"),
        key_line("Shift+G", "Push all"),
        key_line("Shift+D", "Destroy all"),
        key_line("Shift+A", "Spawn agent on all"),
        Line::from(""),
        section_header("Dashboard"),
        key_line("Arrows", "Navigate card grid"),
        key_line("Enter", "Jump to slot detail"),
        Line::from(""),
        section_header("Multiplex Log"),
        key_line("1-6", "Toggle slot visibility"),
        key_line("Tab", "Cycle source filter"),
        key_line("/", "Enter search"),
        key_line("E / W", "Next / prev error"),
        key_line("F", "Re-engage auto-follow"),
        key_line("C", "Clear search"),
        Line::from(""),
        section_header("Dialogs"),
        key_line("Tab", "Next field"),
        key_line("Shift+Tab", "Previous field"),
        key_line("Enter", "Submit / confirm"),
        key_line("Esc", "Cancel / close"),
        key_line("Up/Down", "Navigate list"),
    ];

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn section_header(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("  {title}"),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ))
}

fn key_line(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("    {key:<12}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(desc.to_string(), Style::default().fg(Color::White)),
    ])
}
