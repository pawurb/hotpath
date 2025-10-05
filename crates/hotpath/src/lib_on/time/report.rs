use std::collections::HashMap;
use std::time::Duration;

use super::super::output::{format_function_name, MetricType, MetricsProvider};
use super::state::FunctionStats;

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
        "Execution duration of functions.".to_string()
    }

    fn metric_data(&self) -> HashMap<String, Vec<MetricType>> {
        // Find wrapper function's total value if it exists
        let wrapper_total = self
            .stats
            .iter()
            .find(|(_, s)| s.wrapper)
            .map(|(_, s)| s.total_duration_ns);

        // Use wrapper's total if available, otherwise use total_elapsed
        let reference_total = wrapper_total.unwrap_or(self.total_elapsed.as_nanos() as u64);

        self.stats
            .iter()
            .filter(|(_, s)| s.has_data)
            .map(|(function_name, stats)| {
                let short_name = format_function_name(function_name);

                let percentage = if reference_total > 0 {
                    (stats.total_duration_ns as f64 / reference_total as f64) * 100.0
                } else {
                    0.0
                };

                let mut metrics = vec![
                    MetricType::CallsCount(stats.count),
                    MetricType::DurationNs(stats.avg_duration_ns()),
                ];

                for p in self.percentiles.iter() {
                    let value = stats.percentile(*p as f64);
                    metrics.push(MetricType::DurationNs(value.as_nanos() as u64));
                }

                metrics.push(MetricType::DurationNs(stats.total_duration_ns));
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
