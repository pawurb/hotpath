use super::super::app::App;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::block::BorderType,
    widgets::{Block, Cell, Row, Table},
    Frame,
};

pub(crate) fn render_functions_table(frame: &mut Frame, app: &mut App, area: Rect) {
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

    let header = Row::new(header_cells).height(1);

    let entries = app.get_sorted_entries();

    let rows = entries.iter().map(|(function_name, metrics)| {
        let cells = std::iter::once(Cell::from(function_name.as_str()))
            .chain(metrics.iter().map(|m| Cell::from(format!("{}", m))))
            .collect::<Vec<_>>();

        Row::new(cells)
    });

    let border_type = BorderType::Thick;
    let block_style = Style::default();

    let num_percentiles = app.metrics.percentiles.len();

    let function_pct: u16 = 35;
    let remaining_pct: u16 = 100 - function_pct;
    let num_other_cols = (4 + num_percentiles) as u16; // Calls, Avg, P95s, Total, % Total
    let col_pct: u16 = remaining_pct / num_other_cols;

    let table = Table::new(
        rows,
        vec![Constraint::Percentage(function_pct)] // Function
            .into_iter()
            .chain(vec![
                Constraint::Percentage(col_pct), // Calls
                Constraint::Percentage(col_pct), // Avg
            ])
            .chain((0..num_percentiles).map(|_| Constraint::Percentage(col_pct))) // P95, etc
            .chain(vec![
                Constraint::Percentage(col_pct), // Total
                Constraint::Percentage(col_pct), // % Total
            ])
            .collect::<Vec<_>>(),
    )
    .header(header)
    .block(
        Block::bordered()
            .border_type(border_type)
            .style(block_style)
            .title(Span::styled(
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
