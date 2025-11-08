use super::{app::App, widgets};
use hotpath::MetricType;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

pub fn render_ui(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Status bar
            Constraint::Min(0),    // Main table
            Constraint::Length(3), // Help bar
        ])
        .split(frame.area());

    widgets::render_status_bar(
        frame,
        chunks[0],
        app.paused,
        &app.error_message,
        &app.last_successful_fetch,
        app.last_refresh,
    );

    render_table(frame, app, chunks[1]);

    widgets::render_help_bar(frame, chunks[2]);
}

fn render_table(frame: &mut Frame, app: &mut App, area: Rect) {
    let title = format!(
        " {} - {} ",
        app.metrics.caller_name, app.metrics.description
    );

    let header_cells = vec![
        "Function".to_string(),
        "Calls".to_string(),
        "Avg".to_string(),
    ]
    .into_iter()
    .chain(
        app.metrics
            .percentiles
            .iter()
            .map(|p| format!("P{}", p))
            .collect::<Vec<_>>(),
    )
    .chain(vec!["Total".to_string(), "% Total".to_string()])
    .map(|h| {
        Cell::from(h).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    })
    .collect::<Vec<_>>();

    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let mut entries: Vec<(String, Vec<MetricType>)> = app
        .metrics
        .data
        .0
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    entries.sort_by(|(_, metrics_a), (_, metrics_b)| {
        let percent_a = metrics_a
            .iter()
            .find_map(|m| {
                if let MetricType::Percentage(p) = m {
                    Some(*p)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let percent_b = metrics_b
            .iter()
            .find_map(|m| {
                if let MetricType::Percentage(p) = m {
                    Some(*p)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        percent_b.cmp(&percent_a)
    });

    let rows = entries.iter().map(|(function_name, metrics)| {
        let cells = std::iter::once(Cell::from(function_name.as_str()))
            .chain(metrics.iter().map(|m| Cell::from(format!("{}", m))))
            .collect::<Vec<_>>();

        Row::new(cells)
    });

    let num_percentiles = app.metrics.percentiles.len();
    let table = Table::new(
        rows,
        vec![
            Constraint::Percentage(30), // Function
            Constraint::Length(10),     // Calls
            Constraint::Length(12),     // Avg
        ]
        .into_iter()
        .chain((0..num_percentiles).map(|_| Constraint::Length(12)))
        .chain(vec![
            Constraint::Length(12), // Total
            Constraint::Length(10), // % Total
        ])
        .collect::<Vec<_>>(),
    )
    .header(header)
    .block(
        Block::default().borders(Borders::ALL).title(Span::styled(
            title,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
    )
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol(">> ");

    frame.render_stateful_widget(table, area, &mut app.table_state);
}
