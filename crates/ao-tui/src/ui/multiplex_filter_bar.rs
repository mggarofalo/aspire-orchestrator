use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use ao_core::services::log_tailer::LogSource;

use crate::app::App;

/// High-contrast color palette for slot prefixes.
pub const SLOT_COLORS: [Color; 8] = [
    Color::Cyan,
    Color::Yellow,
    Color::Magenta,
    Color::Green,
    Color::LightBlue,
    Color::LightRed,
    Color::LightCyan,
    Color::LightYellow,
];

/// Render the 2-row filter bar at the top of the multiplex log view.
pub fn render(f: &mut Frame, area: Rect, app: &App) {
    if area.height < 2 {
        return;
    }

    // Row 1: slot toggles + source filter + search text
    let mut row1_spans: Vec<Span> = vec![Span::raw(" ")];

    let slot_names: Vec<String> = app.slots.iter().map(|s| s.name.clone()).collect();
    for (i, name) in slot_names.iter().enumerate() {
        let color_idx = app
            .log_buffer
            .slot_colors
            .get(name)
            .copied()
            .unwrap_or(i as u8 % 8);
        let color = SLOT_COLORS[color_idx as usize % 8];
        let hidden = app.multiplex_filter.hidden_slots.contains(name);

        let style = if hidden {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        };

        row1_spans.push(Span::styled(format!("[{}]", i + 1), style));
        row1_spans.push(Span::styled(name.clone(), style));
        row1_spans.push(Span::raw(" "));
    }

    row1_spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));

    let src_label = match app.multiplex_filter.source_filter {
        None => "Both",
        Some(LogSource::Agent) => "Agent",
        Some(LogSource::Aspire) => "Aspire",
    };
    row1_spans.push(Span::styled(
        format!("Src:{src_label}"),
        Style::default().fg(Color::White),
    ));

    if !app.multiplex_filter.search_text.is_empty() || app.multiplex_filter.search_input_active {
        row1_spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
        let search_style = if app.multiplex_filter.search_input_active {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        row1_spans.push(Span::styled("/", search_style));
        row1_spans.push(Span::styled(
            app.multiplex_filter.search_text.clone(),
            search_style,
        ));
        if app.multiplex_filter.search_filter_mode {
            row1_spans.push(Span::styled(
                " [filter]",
                Style::default().fg(Color::Yellow),
            ));
        }
    }

    let row1 = Line::from(row1_spans);

    // Row 2: keybinding hints
    let row2 = Line::from(vec![
        Span::styled(" 1-6", Style::default().fg(Color::Yellow)),
        Span::styled(" toggle  ", Style::default().fg(Color::DarkGray)),
        Span::styled("Tab", Style::default().fg(Color::Yellow)),
        Span::styled(" source  ", Style::default().fg(Color::DarkGray)),
        Span::styled("/", Style::default().fg(Color::Yellow)),
        Span::styled(" search  ", Style::default().fg(Color::DarkGray)),
        Span::styled("E/W", Style::default().fg(Color::Yellow)),
        Span::styled(" err  ", Style::default().fg(Color::DarkGray)),
        Span::styled("F", Style::default().fg(Color::Yellow)),
        Span::styled(" follow  ", Style::default().fg(Color::DarkGray)),
        Span::styled("M/Esc", Style::default().fg(Color::Yellow)),
        Span::styled(" back", Style::default().fg(Color::DarkGray)),
    ]);

    let paragraph = Paragraph::new(vec![row1, row2]);
    f.render_widget(paragraph, area);
}
