use colored::*;
use prettytable::{Attr, Cell, Row, Table, color};
use std::collections::HashMap;
use std::time::Duration;

use super::FunctionStats;

pub fn display_performance_summary(
    stats: &HashMap<String, FunctionStats>,
    total_elapsed: Duration,
    caller_name: &str,
    percentiles: &[u8],
) {
    let use_colors = std::env::var("NO_COLOR").is_err();

    let mut table = Table::new();
    // Build header row dynamically based on selected percentiles
    let mut header_cells = vec![
        Cell::new("Function"),
        Cell::new("Calls"),
        Cell::new("Min"),
        Cell::new("Max"),
        Cell::new("Avg"),
    ];

    // Add percentile columns
    for &p in percentiles {
        header_cells.push(Cell::new(&format!("P{}", p)));
    }

    header_cells.push(Cell::new("Total"));
    header_cells.push(Cell::new("% Total"));

    if use_colors {
        let styled_cells: Vec<Cell> = header_cells
            .into_iter()
            .map(|cell| {
                cell.with_style(Attr::Bold)
                    .with_style(Attr::ForegroundColor(color::CYAN))
            })
            .collect();
        table.add_row(Row::new(styled_cells));
    } else {
        let styled_cells: Vec<Cell> = header_cells
            .into_iter()
            .map(|cell| cell.with_style(Attr::Bold))
            .collect();
        table.add_row(Row::new(styled_cells));
    }

    let mut sorted_stats: Vec<_> = stats.iter().collect();
    sorted_stats.sort_by(|(_, a), (_, b)| {
        let a_percentage = if total_elapsed.as_nanos() > 0 {
            (a.total_duration.as_nanos() as f64 / total_elapsed.as_nanos() as f64) * 100.0
        } else {
            0.0
        };
        let b_percentage = if total_elapsed.as_nanos() > 0 {
            (b.total_duration.as_nanos() as f64 / total_elapsed.as_nanos() as f64) * 100.0
        } else {
            0.0
        };
        b_percentage
            .partial_cmp(&a_percentage)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (function_name, stats) in sorted_stats {
        let percentage = if total_elapsed.as_nanos() > 0 {
            (stats.total_duration.as_nanos() as f64 / total_elapsed.as_nanos() as f64) * 100.0
        } else {
            0.0
        };

        let parts: Vec<&str> = function_name.split("::").collect();
        let short_name = if parts.len() > 2 {
            parts[parts.len() - 2..].join("::")
        } else {
            function_name.to_string()
        };

        let mut row_cells = vec![
            Cell::new(&short_name),
            Cell::new(&stats.count.to_string()),
            Cell::new(&format!("{:.2?}", stats.min_duration)),
            Cell::new(&format!("{:.2?}", stats.max_duration)),
            Cell::new(&format!("{:.2?}", stats.avg_duration())),
        ];

        // Add percentile values based on selected percentiles
        if !percentiles.is_empty() {
            let mut sorted_measurements = stats.measurements.clone();
            sorted_measurements.sort();

            for &p in percentiles {
                let value = stats.percentile(p as f64, &sorted_measurements);
                row_cells.push(Cell::new(&format!("{:.2?}", value)));
            }
        }

        row_cells.push(Cell::new(&format!("{:.2?}", stats.total_duration)));
        row_cells.push(Cell::new(&format!("{percentage:.2}%")).with_style(Attr::Bold));

        table.add_row(Row::new(row_cells));
    }

    let title = format!(
        "\n{} Performance summary from {} (Total time: {:.2?}):",
        "[hotpath]".blue().bold(),
        caller_name.yellow().bold(),
        total_elapsed
    );
    println!("{title}");
    table.printstd();
}
