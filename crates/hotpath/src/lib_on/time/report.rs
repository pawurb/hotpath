use std::collections::HashMap;
use std::time::Duration;

use super::super::output::{format_function_name, MetricType, MetricsProvider};
use super::state::FunctionStats;
use colored::*;

pub struct StatsData<'a> {
    pub stats: &'a HashMap<&'static str, FunctionStats>,
    pub total_elapsed: Duration,
    pub percentiles: Vec<u8>,
    pub caller_name: String,
}

impl<'a> MetricsProvider<'a> for StatsData<'a> {
    fn new(
        stats: &'a HashMap<&'static str, FunctionStats>,
        total_elapsed: Duration,
        percentiles: Vec<u8>,
        caller_name: String,
    ) -> Self {
        Self {
            stats,
            total_elapsed,
            percentiles,
            caller_name,
        }
    }

    fn percentiles(&self) -> Vec<u8> {
        self.percentiles.clone()
    }

    fn description(&self) -> String {
        format!(
            "\n{} Performance summary from {} (Total time: {:.2?}):",
            "[hotpath]".blue().bold(),
            self.caller_name.yellow().bold(),
            self.total_elapsed
        )
    }

    fn metric_data(&self) -> HashMap<String, Vec<MetricType>> {
        self.stats
            .iter()
            .filter(|(_, s)| s.has_data)
            .map(|(function_name, stats)| {
                let short_name = format_function_name(function_name);

                let percentage = if self.total_elapsed.as_nanos() > 0 {
                    (stats.total_duration_ns as f64 / self.total_elapsed.as_nanos() as f64) * 100.0
                } else {
                    0.0
                };

                let mut metrics = vec![
                    MetricType::CallsCount(stats.count),
                    MetricType::Timing(stats.avg_duration_ns()),
                ];

                for p in self.percentiles.iter() {
                    let value = stats.percentile(*p as f64);
                    metrics.push(MetricType::Timing(value.as_nanos() as u64));
                }

                metrics.push(MetricType::Timing(stats.total_duration_ns));
                metrics.push(MetricType::Percentage((percentage * 100.0) as u64));

                (short_name, metrics)
            })
            .collect()
    }

    fn total_elapsed(&self) -> u64 {
        self.total_elapsed.as_nanos() as u64
    }

    fn caller_name(&self) -> &str {
        &self.caller_name
    }
}
