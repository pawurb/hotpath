mod comment;

use clap::Parser;
use comment::upsert_pr_comment;
use eyre::Result;
use hotpath::{format_bytes, MetricsJson};
use prettytable::{Cell, Row, Table};
use std::env;
use std::fmt;
use std::time::Duration;

#[derive(Debug, Parser)]
pub struct ProfilePrArgs {
    #[arg(long, help = "JSON metrics from head branch")]
    head_metrics: String,

    #[arg(long, help = "JSON metrics from base branch")]
    base_metrics: String,

    #[arg(long, help = "GitHub token for API access")]
    github_token: String,

    #[arg(long, help = "Pull request number")]
    pr_number: String,

    #[arg(
        long,
        help = "Emoji threshold percentage for performance changes (default: 20, use 0 to disable)"
    )]
    emoji_threshold: Option<u32>,
}

impl ProfilePrArgs {
    pub fn run(&self) -> Result<()> {
        let repo = env::var("GITHUB_REPOSITORY").unwrap_or_default();

        if repo.is_empty() || self.pr_number.is_empty() {
            println!("No PR context found, skipping comment posting");
            return Ok(());
        }

        // Convert emoji_threshold: None -> Some(20), Some(0) -> None
        let emoji_threshold = if let Some(0) = self.emoji_threshold {
            None
        } else {
            Some(self.emoji_threshold.unwrap_or(20))
        };

        let head_metrics_data: MetricsJson = serde_json::from_str(&self.head_metrics)
            .map_err(|e| eyre::eyre!("Failed to deserialize head metrics: {}", e))?;
        let base_metrics_data: MetricsJson = serde_json::from_str(&self.base_metrics)
            .map_err(|e| eyre::eyre!("Failed to deserialize base metrics: {}", e))?;

        let comparison = compare_metrics(&base_metrics_data, &head_metrics_data);
        let comparison_markdown =
            format_comparison_markdown(&comparison, &base_metrics_data, emoji_threshold);

        let mut body = comparison_markdown;
        body.push_str("\n<details>\n<summary>📊 View Raw JSON Metrics</summary>\n\n");
        body.push_str("### PR Metrics\n```json\n");
        body.push_str(&serde_json::to_string_pretty(&head_metrics_data)?);
        body.push_str("\n```\n\n### Main Branch Metrics\n```json\n");
        body.push_str(&serde_json::to_string_pretty(&base_metrics_data)?);
        body.push_str("\n```\n</details>\n");

        match upsert_pr_comment(
            &repo,
            &self.pr_number,
            &self.github_token,
            &body,
            &head_metrics_data.hotpath_profiling_mode,
        ) {
            Ok(_) => {}
            Err(e) => println!("Failed to post/update comment: {}", e),
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum MetricDiff {
    CallsCount(u64, u64), // (before, after)
    DurationNs(u64, u64), // (before, after) - Duration in nanoseconds
    AllocBytes(u64, u64), // (before, after) - Bytes allocated
    AllocCount(u64, u64), // (before, after) - Allocation count
    Percentage(u64, u64), // (before, after)
}

impl fmt::Display for MetricDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.format_with_emoji(None))
    }
}

impl MetricDiff {
    fn format_with_emoji(&self, emoji_threshold: Option<u32>) -> String {
        match self {
            MetricDiff::CallsCount(before, after) => {
                let diff_percent = calculate_percentage_diff(*before, *after);
                let emoji = get_emoji_for_diff(diff_percent, emoji_threshold);
                format!("{} → {} ({:+.1}%){}", before, after, diff_percent, emoji)
            }
            MetricDiff::DurationNs(before, after) => {
                let diff_percent = calculate_percentage_diff(*before, *after);
                let before_duration = Duration::from_nanos(*before);
                let after_duration = Duration::from_nanos(*after);
                let emoji = get_emoji_for_diff(diff_percent, emoji_threshold);
                format!(
                    "{:.2?} → {:.2?} ({:+.1}%){}",
                    before_duration, after_duration, diff_percent, emoji
                )
            }
            MetricDiff::AllocBytes(before, after) => {
                let diff_percent = calculate_percentage_diff(*before, *after);
                let emoji = get_emoji_for_diff(diff_percent, emoji_threshold);
                format!(
                    "{} → {} ({:+.1}%){}",
                    format_bytes(*before),
                    format_bytes(*after),
                    diff_percent,
                    emoji
                )
            }
            MetricDiff::AllocCount(before, after) => {
                let diff_percent = calculate_percentage_diff(*before, *after);
                let emoji = get_emoji_for_diff(diff_percent, emoji_threshold);
                format!("{} → {} ({:+.1}%){}", before, after, diff_percent, emoji)
            }
            MetricDiff::Percentage(before, after) => {
                let diff_percent = calculate_percentage_diff(*before, *after);
                let before_percent = *before as f64 / 100.0;
                let after_percent = *after as f64 / 100.0;
                let emoji = get_emoji_for_diff(diff_percent, emoji_threshold);
                format!(
                    "{:.2}% → {:.2}% ({:+.1}%){}",
                    before_percent, after_percent, diff_percent, emoji
                )
            }
        }
    }
}

fn get_emoji_for_diff(diff_percent: f64, threshold: Option<u32>) -> &'static str {
    if let Some(threshold_val) = threshold {
        let threshold = threshold_val as f64;
        if diff_percent > threshold {
            " ⚠️ "
        } else if diff_percent < -threshold {
            " 🚀 "
        } else {
            "   "
        }
    } else {
        ""
    }
}

#[derive(Debug, Clone)]
pub struct MetricsComparison {
    pub total_elapsed_diff: MetricDiff,
    pub function_diffs: Vec<FunctionMetricsDiff>,
}

#[derive(Debug, Clone)]
pub struct FunctionMetricsDiff {
    pub function_name: String,
    pub metrics: Vec<MetricDiff>,
    pub is_removed: bool, // True if function was removed (no longer measured)
    pub is_new: bool,     // True if function is new (not in base)
}

fn calculate_percentage_diff(before: u64, after: u64) -> f64 {
    if before == 0 {
        if after == 0 {
            0.0
        } else {
            100.0 // 100% increase from 0
        }
    } else {
        ((after as f64 - before as f64) / before as f64) * 100.0
    }
}

fn compare_metrics(before_metrics: &MetricsJson, after_metrics: &MetricsJson) -> MetricsComparison {
    use hotpath::MetricType;

    let total_elapsed_diff =
        MetricDiff::DurationNs(before_metrics.total_elapsed, after_metrics.total_elapsed);

    let mut function_diffs = Vec::new();
    let mut new_functions = Vec::new();

    // Process functions that exist in after_metrics (updated, unchanged, or new)
    for (function_name, after_row) in &after_metrics.data.0 {
        if let Some(before_row) = before_metrics.data.0.get(function_name) {
            // Function exists in both before and after - compare metrics
            let mut metrics = Vec::new();

            for (metric_idx, after_metric) in after_row.iter().enumerate() {
                if let Some(before_metric) = before_row.get(metric_idx) {
                    let diff = match (before_metric, after_metric) {
                        (MetricType::CallsCount(before_val), MetricType::CallsCount(after_val)) => {
                            MetricDiff::CallsCount(*before_val, *after_val)
                        }
                        (MetricType::DurationNs(before_val), MetricType::DurationNs(after_val)) => {
                            MetricDiff::DurationNs(*before_val, *after_val)
                        }
                        (MetricType::AllocBytes(before_val), MetricType::AllocBytes(after_val)) => {
                            MetricDiff::AllocBytes(*before_val, *after_val)
                        }
                        (MetricType::AllocCount(before_val), MetricType::AllocCount(after_val)) => {
                            MetricDiff::AllocCount(*before_val, *after_val)
                        }
                        (MetricType::Percentage(before_val), MetricType::Percentage(after_val)) => {
                            MetricDiff::Percentage(*before_val, *after_val)
                        }
                        _ => continue, // Skip mismatched metric types
                    };
                    metrics.push(diff);
                }
            }

            function_diffs.push(FunctionMetricsDiff {
                function_name: function_name.clone(),
                metrics,
                is_removed: false,
                is_new: false,
            });
        } else {
            // Function is new (exists in after but not in before) - show 0 → after
            let mut metrics = Vec::new();

            for after_metric in after_row.iter() {
                let diff = match after_metric {
                    MetricType::CallsCount(after_val) => MetricDiff::CallsCount(0, *after_val),
                    MetricType::DurationNs(after_val) => MetricDiff::DurationNs(0, *after_val),
                    MetricType::AllocBytes(after_val) => MetricDiff::AllocBytes(0, *after_val),
                    MetricType::AllocCount(after_val) => MetricDiff::AllocCount(0, *after_val),
                    MetricType::Percentage(after_val) => MetricDiff::Percentage(0, *after_val),
                    MetricType::Unsupported => continue,
                };
                metrics.push(diff);
            }

            new_functions.push(FunctionMetricsDiff {
                function_name: function_name.clone(),
                metrics,
                is_removed: false,
                is_new: true,
            });
        }
    }

    // Process functions that were removed (exist in before but not in after)
    for (function_name, before_row) in &before_metrics.data.0 {
        // Check if this function exists in after_metrics
        if !after_metrics.data.0.contains_key(function_name) {
            // Function was removed, show before → 0
            let mut metrics = Vec::new();

            for before_metric in before_row.iter() {
                let diff = match before_metric {
                    MetricType::CallsCount(before_val) => MetricDiff::CallsCount(*before_val, 0),
                    MetricType::DurationNs(before_val) => MetricDiff::DurationNs(*before_val, 0),
                    MetricType::AllocBytes(before_val) => MetricDiff::AllocBytes(*before_val, 0),
                    MetricType::AllocCount(before_val) => MetricDiff::AllocCount(*before_val, 0),
                    MetricType::Percentage(before_val) => MetricDiff::Percentage(*before_val, 0),
                    MetricType::Unsupported => continue,
                };
                metrics.push(diff);
            }

            function_diffs.push(FunctionMetricsDiff {
                function_name: function_name.clone(),
                metrics,
                is_removed: true,
                is_new: false,
            });
        }
    }

    function_diffs.extend(new_functions);

    // Sort by percent_total in head branch (after value), descending order
    function_diffs.sort_by(|a, b| {
        let a_percent = a
            .metrics
            .iter()
            .find_map(|m| {
                if let MetricDiff::Percentage(_, after) = m {
                    Some(*after)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let b_percent = b
            .metrics
            .iter()
            .find_map(|m| {
                if let MetricDiff::Percentage(_, after) = m {
                    Some(*after)
                } else {
                    None
                }
            })
            .unwrap_or(0);

        b_percent.cmp(&a_percent)
    });

    MetricsComparison {
        total_elapsed_diff,
        function_diffs,
    }
}

fn format_comparison_markdown(
    comparison: &MetricsComparison,
    metrics: &MetricsJson,
    emoji_threshold: Option<u32>,
) -> String {
    let mut markdown = String::new();

    let base_branch = env::var("GITHUB_BASE_REF").unwrap_or_else(|_| "before".to_string());
    let head_branch = env::var("GITHUB_HEAD_REF").unwrap_or_else(|_| "after".to_string());

    markdown.push_str(&format!(
        "### Performance Comparison `{}` → `{}`\n\n",
        base_branch, head_branch
    ));
    markdown.push_str(&format!(
        "**Total Elapsed Time:** {}\n\n",
        comparison
            .total_elapsed_diff
            .format_with_emoji(emoji_threshold)
    ));
    markdown.push_str(&format!(
        "**Profiling Mode:** {} - {}\n",
        metrics.hotpath_profiling_mode, metrics.description
    ));

    if comparison.function_diffs.is_empty() {
        markdown.push_str("*No functions to compare*\n");
        return markdown;
    }

    let mut table = Table::new();

    let mut header_cells = vec![Cell::new("Function"), Cell::new("Calls"), Cell::new("Avg")];
    for &p in &metrics.percentiles {
        header_cells.push(Cell::new(&format!("P{}", p)));
    }
    header_cells.push(Cell::new("Total"));
    header_cells.push(Cell::new("% Total"));
    table.add_row(Row::new(header_cells));

    for func_diff in &comparison.function_diffs {
        let function_display = if func_diff.is_removed {
            format!("️🗑️ {}", func_diff.function_name)
        } else if func_diff.is_new {
            format!("🆕 {}", func_diff.function_name)
        } else {
            func_diff.function_name.clone()
        };

        let mut row_cells = vec![Cell::new(&function_display)];
        for metric_diff in &func_diff.metrics {
            row_cells.push(Cell::new(&metric_diff.format_with_emoji(emoji_threshold)));
        }
        table.add_row(Row::new(row_cells));
    }

    markdown.push_str("```\n");
    markdown.push_str(&table.to_string());
    markdown.push_str("```\n\n");

    markdown.push_str("---\n");
    markdown.push_str("*Generated with [hotpath](https://github.com/pawurb/hotpath/)*\n");

    markdown
}

#[cfg(test)]
mod test {
    use super::*;
    use hotpath::{
        MetricType::{CallsCount, DurationNs, Percentage},
        MetricsDataJson,
    };

    #[test]
    fn test_format_comparison_markdown() {
        use std::collections::HashMap;

        let mut pr_data = HashMap::new();
        pr_data.insert(
            "basic::async_function".to_string(),
            vec![
                CallsCount(100),
                DurationNs(1256314),
                DurationNs(1276927),
                DurationNs(125631441),
                Percentage(8940),
            ],
        );
        pr_data.insert(
            "basic::sync_function".to_string(),
            vec![
                CallsCount(100),
                DurationNs(61184),
                DurationNs(62847),
                DurationNs(6118443),
                Percentage(435),
            ],
        );
        pr_data.insert(
            "custom_block".to_string(),
            vec![
                CallsCount(100),
                DurationNs(62036),
                DurationNs(64031),
                DurationNs(6203646),
                Percentage(441),
            ],
        );

        let pr_metrics = MetricsJson {
            hotpath_profiling_mode: hotpath::ProfilingMode::Timing,
            total_elapsed: 140515884,
            caller_name: "basic::main".to_string(),
            percentiles: vec![95],
            description: "Time metrics".to_string(),
            data: MetricsDataJson(pr_data),
        };

        let mut main_data = HashMap::new();
        main_data.insert(
            "basic::async_function".to_string(),
            vec![
                CallsCount(90),
                DurationNs(1130683),
                DurationNs(1149234),
                DurationNs(113068297),
                Percentage(8046),
            ],
        );
        main_data.insert(
            "basic::sync_function".to_string(),
            vec![
                CallsCount(90),
                DurationNs(55066),
                DurationNs(56562),
                DurationNs(5506599),
                Percentage(392),
            ],
        );
        main_data.insert(
            "custom_block".to_string(),
            vec![
                CallsCount(90),
                DurationNs(55832),
                DurationNs(57628),
                DurationNs(5583281),
                Percentage(397),
            ],
        );

        let main_metrics = MetricsJson {
            hotpath_profiling_mode: hotpath::ProfilingMode::Timing,
            total_elapsed: 126464296,
            caller_name: "basic::main".to_string(),
            percentiles: vec![95],
            description: "Time metrics".to_string(),
            data: MetricsDataJson(main_data),
        };

        let comparison = compare_metrics(&main_metrics, &pr_metrics);

        println!("Total elapsed time diff: {}", comparison.total_elapsed_diff);

        for function_diff in &comparison.function_diffs {
            println!("Function: {}", function_diff.function_name);
            for metric_diff in &function_diff.metrics {
                println!("  {}", metric_diff);
            }
        }

        // Test markdown formatting
        let markdown = format_comparison_markdown(&comparison, &main_metrics, Some(20));
        println!("\n=== Generated Markdown ===\n{}", markdown);
    }

    #[test]
    fn test_removed_function() {
        use hotpath::MetricType::{CallsCount, DurationNs, Percentage};
        use std::collections::HashMap;

        let mut pr_data = HashMap::new();
        pr_data.insert(
            "test::function_a".to_string(),
            vec![
                CallsCount(100),
                DurationNs(1000000),
                DurationNs(1100000),
                DurationNs(100000000),
                Percentage(10000),
            ],
        );

        let pr_metrics = MetricsJson {
            hotpath_profiling_mode: hotpath::ProfilingMode::Timing,
            total_elapsed: 100000000,
            caller_name: "test::main".to_string(),
            percentiles: vec![95],
            description: "Time metrics".to_string(),
            data: MetricsDataJson(pr_data),
        };

        let mut main_data = HashMap::new();
        main_data.insert(
            "test::function_a".to_string(),
            vec![
                CallsCount(90),
                DurationNs(900000),
                DurationNs(1000000),
                DurationNs(81000000),
                Percentage(9000),
            ],
        );
        main_data.insert(
            "test::function_b".to_string(),
            vec![
                CallsCount(50),
                DurationNs(500000),
                DurationNs(550000),
                DurationNs(25000000),
                Percentage(2500),
            ],
        );

        let main_metrics = MetricsJson {
            hotpath_profiling_mode: hotpath::ProfilingMode::Timing,
            total_elapsed: 120000000,
            caller_name: "test::main".to_string(),
            percentiles: vec![95],
            description: "Time metrics".to_string(),
            data: MetricsDataJson(main_data),
        };

        let comparison = compare_metrics(&main_metrics, &pr_metrics);

        println!("\n=== Test Removed Function ===");
        println!("Total elapsed time diff: {}", comparison.total_elapsed_diff);

        for function_diff in &comparison.function_diffs {
            println!(
                "Function: {} (removed: {})",
                function_diff.function_name, function_diff.is_removed
            );
            for metric_diff in &function_diff.metrics {
                println!("  {}", metric_diff);
            }
        }

        let markdown = format_comparison_markdown(&comparison, &main_metrics, Some(20));
        println!("\n=== Generated Markdown ===\n{}", markdown);

        assert!(comparison
            .function_diffs
            .iter()
            .any(|f| f.function_name == "test::function_b" && f.is_removed));
    }

    #[test]
    fn test_new_function() {
        use hotpath::MetricType::{CallsCount, DurationNs, Percentage};
        use std::collections::HashMap;

        let mut pr_data = HashMap::new();
        pr_data.insert(
            "test::function_a".to_string(),
            vec![
                CallsCount(100),
                DurationNs(1000000),
                DurationNs(1100000),
                DurationNs(100000000),
                Percentage(8000),
            ],
        );
        pr_data.insert(
            "test::function_c".to_string(),
            vec![
                CallsCount(60),
                DurationNs(600000),
                DurationNs(650000),
                DurationNs(36000000),
                Percentage(2400),
            ],
        );

        let pr_metrics = MetricsJson {
            hotpath_profiling_mode: hotpath::ProfilingMode::Timing,
            total_elapsed: 150000000,
            caller_name: "test::main".to_string(),
            percentiles: vec![95],
            description: "Time metrics".to_string(),
            data: MetricsDataJson(pr_data),
        };

        let mut main_data = HashMap::new();
        main_data.insert(
            "test::function_a".to_string(),
            vec![
                CallsCount(90),
                DurationNs(900000),
                DurationNs(1000000),
                DurationNs(81000000),
                Percentage(9000),
            ],
        );

        let main_metrics = MetricsJson {
            hotpath_profiling_mode: hotpath::ProfilingMode::Timing,
            total_elapsed: 120000000,
            caller_name: "test::main".to_string(),
            percentiles: vec![95],
            description: "Time metrics".to_string(),
            data: MetricsDataJson(main_data),
        };

        let comparison = compare_metrics(&main_metrics, &pr_metrics);

        println!("\n=== Test New Function ===");
        println!("Total elapsed time diff: {}", comparison.total_elapsed_diff);

        for function_diff in &comparison.function_diffs {
            println!(
                "Function: {} (new: {}, removed: {})",
                function_diff.function_name, function_diff.is_new, function_diff.is_removed
            );
            for metric_diff in &function_diff.metrics {
                println!("  {}", metric_diff);
            }
        }

        let markdown = format_comparison_markdown(&comparison, &main_metrics, Some(20));
        println!("\n=== Generated Markdown ===\n{}", markdown);

        assert!(comparison
            .function_diffs
            .iter()
            .any(|f| f.function_name == "test::function_c" && f.is_new));
    }

    #[test]
    fn test_new_and_removed_functions() {
        use hotpath::MetricType::{CallsCount, DurationNs, Percentage};
        use std::collections::HashMap;

        // Head has function_a (updated) and function_c (new)
        let mut pr_data = HashMap::new();
        pr_data.insert(
            "test::function_a".to_string(),
            vec![
                CallsCount(100),
                DurationNs(1000000),
                DurationNs(1100000),
                DurationNs(100000000),
                Percentage(7000),
            ],
        );
        pr_data.insert(
            "test::function_c".to_string(),
            vec![
                CallsCount(40),
                DurationNs(400000),
                DurationNs(450000),
                DurationNs(16000000),
                Percentage(1500),
            ],
        );

        let pr_metrics = MetricsJson {
            hotpath_profiling_mode: hotpath::ProfilingMode::Timing,
            total_elapsed: 140000000,
            caller_name: "test::main".to_string(),
            percentiles: vec![95],
            description: "Time metrics".to_string(),
            data: MetricsDataJson(pr_data),
        };

        // Base has function_a (updated) and function_b (removed)
        let mut main_data = HashMap::new();
        main_data.insert(
            "test::function_a".to_string(),
            vec![
                CallsCount(90),
                DurationNs(900000),
                DurationNs(1000000),
                DurationNs(81000000),
                Percentage(8000),
            ],
        );
        main_data.insert(
            "test::function_b".to_string(),
            vec![
                CallsCount(30),
                DurationNs(300000),
                DurationNs(350000),
                DurationNs(9000000),
                Percentage(1200),
            ],
        );

        let main_metrics = MetricsJson {
            hotpath_profiling_mode: hotpath::ProfilingMode::Timing,
            total_elapsed: 120000000,
            caller_name: "test::main".to_string(),
            percentiles: vec![95],
            description: "Time metrics".to_string(),
            data: MetricsDataJson(main_data),
        };

        let comparison = compare_metrics(&main_metrics, &pr_metrics);

        println!("\n=== Test New and Removed Functions ===");
        println!("Total elapsed time diff: {}", comparison.total_elapsed_diff);

        for function_diff in &comparison.function_diffs {
            println!(
                "Function: {} (new: {}, removed: {})",
                function_diff.function_name, function_diff.is_new, function_diff.is_removed
            );
        }

        // Test markdown formatting
        let markdown = format_comparison_markdown(&comparison, &main_metrics, Some(20));
        println!("\n=== Generated Markdown ===\n{}", markdown);

        // Verify we have both new and removed functions
        assert_eq!(comparison.function_diffs.len(), 3); // a (updated), b (removed), c (new)
        assert!(comparison
            .function_diffs
            .iter()
            .any(|f| f.function_name == "test::function_b" && f.is_removed));
        assert!(comparison
            .function_diffs
            .iter()
            .any(|f| f.function_name == "test::function_c" && f.is_new));
        assert!(comparison
            .function_diffs
            .iter()
            .any(|f| f.function_name == "test::function_a" && !f.is_new && !f.is_removed));
    }
}
