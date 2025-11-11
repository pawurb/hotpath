use super::super::app::Focus;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub(crate) fn render_help_bar(frame: &mut Frame, area: Rect, focus: Focus, show_samples: bool) {
    let mut spans = vec![
        Span::raw("Quit "),
        Span::styled(
            "<q>",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Navigate "),
        Span::styled(
            "<↑/k ↓/j>",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" | Toggle Samples "),
        Span::styled(
            "<o>",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ];

    // Show Tab hint only when samples panel is visible
    if show_samples {
        spans.push(Span::raw(" | Switch Focus "));
        spans.push(Span::styled(
            "<Tab>",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans.push(Span::raw(" | Pause "));
    spans.push(Span::styled(
        "<p>",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    ));

    // Show current focus when samples panel is visible
    let title = if show_samples {
        match focus {
            Focus::Functions => " Controls [Functions] ",
            Focus::Samples => " Controls [Samples] ",
        }
    } else {
        " Controls "
    };

    let help_text = vec![Line::from(spans)];

    let help_paragraph =
        Paragraph::new(help_text).block(Block::default().borders(Borders::ALL).title(title));

    frame.render_widget(help_paragraph, area);
}
