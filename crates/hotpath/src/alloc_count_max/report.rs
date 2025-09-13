use colored::*;
use std::collections::HashMap;
use std::time::Duration;

use super::state::FunctionStats;
use crate::Tableable;

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

    fn description(&self, caller_name: &str) -> String {
        format!(
            "\n{} Max count allocation statistics from {} (Total time: {:.2?}):",
            "[hotpath]".blue().bold(),
            caller_name.yellow().bold(),
            self.total_elapsed
        )
    }

    fn percentiles(&self) -> Vec<u8> {
        self.percentiles.clone()
    }

    fn has_unsupported_async(&self) -> bool {
        self.stats.values().any(|s| s.has_unsupported_async)
    }

    fn rows(&self) -> Vec<Vec<String>> {
        let mut sorted_stats: Vec<_> = self.stats.iter().filter(|(_, s)| s.has_data).collect();

        // Calculate total count across all functions for percentage calculation
        let grand_total_count: u64 = sorted_stats
            .iter()
            .map(|(_, stats)| stats.total_count())
            .sum();

        // Sort by % Total descending
        sorted_stats.sort_by(|(_, a), (_, b)| {
            let a_percentage = if grand_total_count > 0 {
                (a.total_count() as f64 / grand_total_count as f64) * 100.0
            } else {
                0.0
            };
            let b_percentage = if grand_total_count > 0 {
                (b.total_count() as f64 / grand_total_count as f64) * 100.0
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
                let percentage = if grand_total_count > 0 {
                    (stats.total_count() as f64 / grand_total_count as f64) * 100.0
                } else {
                    0.0
                };

                let parts: Vec<&str> = function_name.split("::").collect();
                let short_name = if parts.len() > 2 {
                    parts[parts.len() - 2..].join("::")
                } else {
                    function_name.to_string()
                };

                let mut row = if stats.has_unsupported_async {
                    vec![short_name, stats.count.to_string(), "N/A*".to_string()]
                } else {
                    vec![
                        short_name,
                        stats.count.to_string(),
                        stats.avg_count().to_string(),
                    ]
                };

                for &p in &self.percentiles {
                    if stats.has_unsupported_async {
                        row.push("N/A*".to_string());
                    } else {
                        let count_max = stats.count_max_percentile(p as f64);
                        row.push(count_max.to_string());
                    }
                }

                if stats.has_unsupported_async {
                    row.push("N/A*".to_string());
                    row.push("N/A*".to_string());
                } else {
                    row.push(stats.total_count().to_string());
                    row.push(format!("{:.2}%", percentage));
                }

                row
            })
            .collect()
    }
}
