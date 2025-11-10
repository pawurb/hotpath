use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::time::Instant;

pub(crate) fn render_status_bar(
    frame: &mut Frame,
    area: Rect,
    paused: bool,
    error_message: &Option<String>,
    last_successful_fetch: &Option<Instant>,
    last_refresh: Instant,
) {
    let status_text = if let Some(error) = error_message {
        let time_since_success = last_successful_fetch
            .map(|t| format!("{}s ago", t.elapsed().as_secs()))
            .unwrap_or_else(|| "never".to_string());

        vec![Line::from(vec![
            Span::styled(
                "⚠ Error: ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(error),
            Span::raw(" (last success: "),
            Span::raw(time_since_success),
            Span::raw(")"),
        ])]
    } else {
        let refresh_time = last_refresh.elapsed().as_secs();
        let status_symbol = if paused { "⏸ Paused" } else { "✓ Live" };
        let status_color = if paused { Color::Yellow } else { Color::Green };

        vec![Line::from(vec![
            Span::styled(
                status_symbol,
                Style::default()
                    .fg(status_color)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" (refreshed {}s ago)", refresh_time)),
        ])]
    };

    let status_paragraph =
        Paragraph::new(status_text).block(Block::default().borders(Borders::ALL).title(" Status "));

    frame.render_widget(status_paragraph, area);
}
