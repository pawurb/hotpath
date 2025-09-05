use colored::*;
use prettytable::{Attr, Cell, Row, Table, color};
use std::collections::HashMap;
use std::time::Duration;

use super::FunctionStats;

pub fn display_performance_summary(
    stats: &HashMap<String, FunctionStats>,
    total_elapsed: Duration,
    caller_name: &str,
) {
    let use_colors = std::env::var("NO_COLOR").is_err();

    let mut table = Table::new();
    if use_colors {
        table.add_row(Row::new(vec![
            Cell::new("Function")
                .with_style(Attr::Bold)
                .with_style(Attr::ForegroundColor(color::CYAN)),
            Cell::new("Calls")
                .with_style(Attr::Bold)
                .with_style(Attr::ForegroundColor(color::CYAN)),
            Cell::new("Min")
                .with_style(Attr::Bold)
                .with_style(Attr::ForegroundColor(color::CYAN)),
            Cell::new("Max")
                .with_style(Attr::Bold)
                .with_style(Attr::ForegroundColor(color::CYAN)),
            Cell::new("Avg")
                .with_style(Attr::Bold)
                .with_style(Attr::ForegroundColor(color::CYAN)),
            Cell::new("Total")
                .with_style(Attr::Bold)
                .with_style(Attr::ForegroundColor(color::CYAN)),
            Cell::new("% Total")
                .with_style(Attr::Bold)
                .with_style(Attr::ForegroundColor(color::CYAN)),
        ]));
    } else {
        table.add_row(Row::new(vec![
            Cell::new("Function").with_style(Attr::Bold),
            Cell::new("Calls").with_style(Attr::Bold),
            Cell::new("Min").with_style(Attr::Bold),
            Cell::new("Max").with_style(Attr::Bold),
            Cell::new("Avg").with_style(Attr::Bold),
            Cell::new("Total").with_style(Attr::Bold),
            Cell::new("% Total").with_style(Attr::Bold),
        ]));
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

        table.add_row(Row::new(vec![
            Cell::new(function_name),
            Cell::new(&stats.count.to_string()),
            Cell::new(&format!("{:.2?}", stats.min_duration)),
            Cell::new(&format!("{:.2?}", stats.max_duration)),
            Cell::new(&format!("{:.2?}", stats.avg_duration())),
            Cell::new(&format!("{:.2?}", stats.total_duration)),
            Cell::new(&format!("{percentage:.2}%")).with_style(Attr::Bold),
        ]));
    }

    let title = format!(
        "\n{} Performance Summary from {} (Total time: {:.2?}):",
        "[hotpath]".blue().bold(),
        caller_name.yellow().bold(),
        total_elapsed
    );
    println!("{title}");
    table.printstd();
}
