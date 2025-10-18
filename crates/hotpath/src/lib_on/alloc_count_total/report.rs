use std::collections::HashMap;
use std::time::Duration;

use super::super::output::{format_function_name, MetricType, MetricsProvider};
use super::state::FunctionStats;
use crate::ProfilingMode;

pub struct StatsData<'a> {
    pub stats: &'a HashMap<&'static str, FunctionStats>,
    pub total_elapsed: Duration,
    pub percentiles: Vec<u8>,
    pub caller_name: &'static str,
    pub limit: usize,
}

impl<'a> MetricsProvider<'a> for StatsData<'a> {
    fn new(
        stats: &'a HashMap<&'static str, FunctionStats>,
        total_elapsed: Duration,
        percentiles: Vec<u8>,
        caller_name: &'static str,
        limit: usize,
    ) -> Self {
        Self {
            stats,
            total_elapsed,
            percentiles,
            caller_name,
            limit,
        }
    }

    fn percentiles(&self) -> Vec<u8> {
        self.percentiles.clone()
    }

    fn profiling_mode(&self) -> ProfilingMode {
        ProfilingMode::AllocCountTotal
    }

    fn has_unsupported_async(&self) -> bool {
        self.stats.values().any(|s| s.has_unsupported_async)
    }

    fn description(&self) -> String {
        "Total number of heap allocations during each function call.".to_string()
    }

    fn metric_data(&self) -> HashMap<String, Vec<MetricType>> {
        let mut filtered_stats: Vec<_> = self.stats.iter().filter(|(_, s)| s.has_data).collect();

        filtered_stats.sort_by(|a, b| b.1.total_count().cmp(&a.1.total_count()));

        let filtered_stats = if self.limit > 0 {
            filtered_stats
                .into_iter()
                .take(self.limit)
                .collect::<Vec<_>>()
        } else {
            filtered_stats
        };

        let wrapper_total_count = self
            .stats
            .iter()
            .find(|(_, s)| s.wrapper)
            .map(|(_, s)| s.total_count());

        let grand_total_count: u64 = wrapper_total_count.unwrap_or_else(|| {
            filtered_stats
                .iter()
                .map(|(_, stats)| stats.total_count())
                .sum()
        });

        filtered_stats
            .into_iter()
            .map(|(function_name, stats)| {
                let short_name = format_function_name(function_name);

                let percentage = if grand_total_count > 0 {
                    (stats.total_count() as f64 / grand_total_count as f64) * 100.0
                } else {
                    0.0
                };

                let mut metrics = if stats.has_unsupported_async {
                    vec![MetricType::CallsCount(stats.count), MetricType::Unsupported]
                } else {
                    vec![
                        MetricType::CallsCount(stats.count),
                        MetricType::AllocCount(stats.avg_count()),
                    ]
                };

                for &p in &self.percentiles {
                    if stats.has_unsupported_async {
                        metrics.push(MetricType::Unsupported);
                    } else {
                        let count_total = stats.count_total_percentile(p as f64);
                        metrics.push(MetricType::AllocCount(count_total));
                    }
                }

                if stats.has_unsupported_async {
                    metrics.push(MetricType::Unsupported);
                    metrics.push(MetricType::Unsupported);
                } else {
                    metrics.push(MetricType::AllocCount(stats.total_count()));
                    metrics.push(MetricType::Percentage((percentage * 100.0) as u64));
                }

                (short_name, metrics)
            })
            .collect()
    }

    fn total_elapsed(&self) -> u64 {
        self.total_elapsed.as_nanos() as u64
    }

    fn caller_name(&self) -> &str {
        self.caller_name
    }

    fn entry_counts(&self) -> (usize, usize) {
        let total_count = self.stats.iter().filter(|(_, s)| s.has_data).count();

        let displayed_count = if self.limit > 0 && self.limit < total_count {
            self.limit
        } else {
            total_count
        };

        (displayed_count, total_count)
    }
}
