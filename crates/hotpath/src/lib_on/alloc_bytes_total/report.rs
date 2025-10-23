use crate::ProfilingMode;
use std::collections::HashMap;
use std::time::Duration;

use super::super::output::{format_function_name, MetricType, MetricsProvider};
use super::state::FunctionStats;

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

    fn profiling_mode(&self) -> ProfilingMode {
        ProfilingMode::AllocBytesTotal
    }

    fn description(&self) -> String {
        #[cfg(feature = "hotpath-alloc-self")]
        {
            "Exclusive bytes allocated by each function (excluding nested calls).".to_string()
        }
        #[cfg(not(feature = "hotpath-alloc-self"))]
        {
            "Cumulative bytes allocated during each function call (including nested calls)."
                .to_string()
        }
    }

    fn percentiles(&self) -> Vec<u8> {
        self.percentiles.clone()
    }

    fn has_unsupported_async(&self) -> bool {
        self.stats.values().any(|s| s.has_unsupported_async)
    }

    fn metric_data(&self) -> HashMap<String, Vec<MetricType>> {
        let mut filtered_stats: Vec<_> = self
            .stats
            .iter()
            .filter(|(_, s)| s.has_data && !(s.wrapper && s.cross_thread))
            .collect();

        filtered_stats.sort_by(|a, b| b.1.total_bytes().cmp(&a.1.total_bytes()));

        let filtered_stats = if self.limit > 0 {
            filtered_stats
                .into_iter()
                .take(self.limit)
                .collect::<Vec<_>>()
        } else {
            filtered_stats
        };

        #[cfg(feature = "hotpath-alloc-self")]
        let grand_total_bytes: u64 = {
            self.stats
                .iter()
                .filter(|(_, s)| s.has_data)
                .map(|(_, stats)| stats.total_bytes())
                .sum()
        };

        #[cfg(not(feature = "hotpath-alloc-self"))]
        let grand_total_bytes: u64 = {
            let has_cross_thread_wrapper =
                self.stats.iter().any(|(_, s)| s.wrapper && s.cross_thread);

            if has_cross_thread_wrapper {
                // If wrapper was moved across threads, use sum of all functions
                filtered_stats
                    .iter()
                    .filter(|(_, s)| !s.wrapper)
                    .map(|(_, stats)| stats.total_bytes())
                    .sum()
            } else {
                // Use wrapper total if available
                let wrapper_total_bytes = self
                    .stats
                    .iter()
                    .find(|(_, s)| s.wrapper)
                    .map(|(_, s)| s.total_bytes());

                wrapper_total_bytes.unwrap_or_else(|| {
                    filtered_stats
                        .iter()
                        .map(|(_, stats)| stats.total_bytes())
                        .sum()
                })
            }
        };

        filtered_stats
            .into_iter()
            .map(|(function_name, stats)| {
                let short_name = format_function_name(function_name);

                let percentage = if grand_total_bytes > 0 {
                    (stats.total_bytes() as f64 / grand_total_bytes as f64) * 100.0
                } else {
                    0.0
                };

                let mut metrics = if stats.has_unsupported_async || stats.cross_thread {
                    vec![MetricType::CallsCount(stats.count), MetricType::Unsupported]
                } else {
                    vec![
                        MetricType::CallsCount(stats.count),
                        MetricType::AllocBytes(stats.avg_bytes()),
                    ]
                };

                for &p in &self.percentiles {
                    if stats.has_unsupported_async || stats.cross_thread {
                        metrics.push(MetricType::Unsupported);
                    } else {
                        let bytes_total = stats.bytes_total_percentile(p as f64);
                        metrics.push(MetricType::AllocBytes(bytes_total));
                    }
                }

                if stats.has_unsupported_async || stats.cross_thread {
                    metrics.push(MetricType::Unsupported);
                    metrics.push(MetricType::Unsupported);
                } else {
                    metrics.push(MetricType::AllocBytes(stats.total_bytes()));
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
        let total_count = self
            .stats
            .iter()
            .filter(|(_, s)| s.has_data && !(s.wrapper && s.cross_thread))
            .count();

        let displayed_count = if self.limit > 0 && self.limit < total_count {
            self.limit
        } else {
            total_count
        };

        (displayed_count, total_count)
    }
}
