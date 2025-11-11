use super::super::app::{App, Focus};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::block::BorderType,
    widgets::{Block, List, ListItem},
    Frame,
};

pub(crate) fn render_samples_panel(frame: &mut Frame, area: Rect, app: &App, focus: Focus) {
    let title = if let Some(ref samples) = app.current_samples {
        format!(" {} ", samples.function_name)
    } else if app.selected_function_name().is_some() {
        " Loading... ".to_string()
    } else {
        " Recent Samples ".to_string()
    };

    let is_focused = matches!(focus, Focus::Samples);

    let border_type = if is_focused {
        BorderType::Thick
    } else {
        BorderType::Plain
    };

    let block_style = if is_focused {
        Style::default()
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::bordered()
        .border_type(border_type)
        .style(block_style)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::BOLD),
        ));

    if let Some(ref samples_data) = app.current_samples {
        // Display actual samples
        let items: Vec<ListItem> = samples_data
            .samples
            .iter()
            .enumerate()
            .map(|(idx, &value)| {
                let formatted_value =
                    format_sample_value(value, &app.metrics.hotpath_profiling_mode);
                let line = format!("  {:>4}.  {}", idx + 1, formatted_value);
                ListItem::new(Line::from(Span::styled(
                    line,
                    Style::default().fg(Color::Cyan),
                )))
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    } else if app.selected_function_name().is_some() {
        // No samples yet
        let items = vec![
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(Span::styled(
                "  Loading samples...",
                Style::default().fg(Color::Gray),
            ))),
        ];
        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    } else {
        // No function selected
        let items = vec![
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(Span::styled(
                "  No function selected",
                Style::default().fg(Color::Gray),
            ))),
            ListItem::new(Line::from("")),
            ListItem::new(Line::from(Span::styled(
                "  Navigate the function list to see samples.",
                Style::default().fg(Color::DarkGray),
            ))),
        ];
        let list = List::new(items).block(block);
        frame.render_widget(list, area);
    }
}

fn format_sample_value(value: u64, profiling_mode: &hotpath::ProfilingMode) -> String {
    match profiling_mode {
        hotpath::ProfilingMode::Timing => hotpath::format_duration(value),
        hotpath::ProfilingMode::AllocBytesTotal => hotpath::format_bytes(value),
        hotpath::ProfilingMode::AllocCountTotal => {
            format!("{}", value)
        }
    }
}
