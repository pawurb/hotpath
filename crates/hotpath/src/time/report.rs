use std::collections::HashMap;
use std::time::Duration;

use super::state::FunctionStats;
use crate::output::Tableable;
use colored::*;

pub struct StatsTable<'a> {
    stats: &'a HashMap<&'static str, FunctionStats>,
    total_elapsed: Duration,
    percentiles: Vec<u8>,
}

impl<'a> Tableable<'a> for StatsTable<'a> {
    fn new(
        stats: &'a HashMap<&'static str, FunctionStats>,
        total_elapsed: Duration,
        percentiles: Vec<u8>,
    ) -> Self {
        Self {
            stats,
            total_elapsed,
            percentiles,
        }
    }

    fn percentiles(&self) -> Vec<u8> {
        self.percentiles.clone()
    }

    fn description(&self, caller_name: &str) -> String {
        format!(
            "\n{} Performance summary from {} (Total time: {:.2?}):",
            "[hotpath]".blue().bold(),
            caller_name.yellow().bold(),
            self.total_elapsed
        )
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let mut sorted_stats: Vec<_> = self.stats.iter().filter(|(_, s)| s.has_data).collect();
        sorted_stats.sort_by(|(_, a), (_, b)| {
            let a_percentage = if self.total_elapsed.as_nanos() > 0 {
                (a.total_duration_ns as f64 / self.total_elapsed.as_nanos() as f64) * 100.0
            } else {
                0.0
            };
            let b_percentage = if self.total_elapsed.as_nanos() > 0 {
                (b.total_duration_ns as f64 / self.total_elapsed.as_nanos() as f64) * 100.0
            } else {
                0.0
            };
            b_percentage
                .partial_cmp(&a_percentage)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        sorted_stats
            .into_iter()
            .map(|(function_name, stats)| {
                let percentage = if self.total_elapsed.as_nanos() > 0 {
                    (stats.total_duration_ns as f64 / self.total_elapsed.as_nanos() as f64) * 100.0
                } else {
                    0.0
                };

                let parts: Vec<&str> = function_name.split("::").collect();
                let short_name = if parts.len() > 2 {
                    parts[parts.len() - 2..].join("::")
                } else {
                    function_name.to_string()
                };

                let mut row = vec![
                    short_name,
                    stats.count.to_string(),
                    format!("{:.2?}", Duration::from_nanos(stats.avg_duration_ns())),
                ];

                for p in self.percentiles.iter() {
                    let value = stats.percentile(*p as f64);
                    row.push(format!("{:.2?}", value));
                }

                row.push(format!(
                    "{:.2?}",
                    Duration::from_nanos(stats.total_duration_ns)
                ));
                row.push(format!("{percentage:.2}%"));

                row
            })
            .collect()
    }
}
