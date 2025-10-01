use super::FunctionStats;
use colored::*;
use prettytable::{color, Attr, Cell, Row, Table};
use serde::{
    ser::{SerializeMap, Serializer},
    Deserialize, Serialize,
};
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

/// Represents different types of profiling metrics with their values.
///
/// This enum wraps metric values with type information, allowing the reporting
/// system to format and display them appropriately. Values are stored in their
/// raw form and formatted when displayed.
///
/// # Variants
///
/// * `CallsCount(u64)` - Number of function calls
/// * `DurationNs(u64)` - Duration in nanoseconds (formatted as human-readable time)
/// * `AllocBytes(u64)` - Bytes allocated (formatted with KB/MB/GB units)
/// * `AllocCount(u64)` - Allocation count
/// * `Percentage(u64)` - Percentage as basis points (1% = 100, formatted as percentage)
/// * `Unsupported` - For N/A values (e.g., async functions when allocation profiling not supported)
///
/// # Examples
///
/// ```rust
/// use hotpath::MetricType;
///
/// let duration = MetricType::DurationNs(1_500_000); // 1.5ms
/// let memory = MetricType::AllocBytes(2048); // 2KB
/// let percent = MetricType::Percentage(9500); // 95.00%
///
/// println!("{}", duration); // Displays: "1.50ms"
/// println!("{}", memory);   // Displays: "2.0 KB"
/// println!("{}", percent);  // Displays: "95.00%"
/// ```
#[derive(Debug, Clone)]
pub enum MetricType {
    CallsCount(u64), // Number of function calls
    DurationNs(u64), // Duration in nanoseconds
    AllocBytes(u64), // Bytes allocated
    AllocCount(u64), // Allocation count
    Percentage(u64), // Percentage as basis points (1% = 100)
    Unsupported,     // For N/A values (async functions when not supported)
}

impl Serialize for MetricType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            MetricType::CallsCount(count) => serializer.serialize_u64(*count),
            MetricType::DurationNs(ns) => serializer.serialize_u64(*ns),
            MetricType::AllocBytes(bytes) => serializer.serialize_u64(*bytes),
            MetricType::AllocCount(count) => serializer.serialize_u64(*count),
            MetricType::Percentage(basis_points) => serializer.serialize_u64(*basis_points),
            MetricType::Unsupported => serializer.serialize_none(),
        }
    }
}

impl fmt::Display for MetricType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetricType::CallsCount(count) => {
                write!(f, "{}", count)
            }
            MetricType::DurationNs(ns) => {
                let duration = Duration::from_nanos(*ns);
                write!(f, "{:.2?}", duration)
            }
            MetricType::AllocBytes(bytes) => {
                write!(f, "{}", format_bytes(*bytes))
            }
            MetricType::AllocCount(count) => {
                write!(f, "{}", count)
            }
            MetricType::Percentage(basis_points) => {
                write!(f, "{:.2}%", *basis_points as f64 / 100.0)
            }
            MetricType::Unsupported => {
                write!(f, "N/A*")
            }
        }
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let bytes_f = bytes as f64;
    let unit_index = (bytes_f.log(THRESHOLD).floor() as usize).min(UNITS.len() - 1);
    let unit_value = bytes_f / THRESHOLD.powi(unit_index as i32);

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", unit_value, UNITS[unit_index])
    }
}

pub(crate) fn format_function_name(function_name: &str) -> String {
    let parts: Vec<&str> = function_name.split("::").collect();
    if parts.len() > 2 {
        parts[parts.len() - 2..].join("::")
    } else {
        function_name.to_string()
    }
}

/// Trait for implementing custom profiling report output.
///
/// Implement this trait to control how profiling results are displayed or stored.
/// Custom reporters can integrate hotpath with logging systems, CI pipelines,
/// monitoring tools, or custom file formats.
///
/// # Examples
///
/// ```rust
/// use hotpath::{Reporter, MetricsProvider};
/// use std::error::Error;
///
/// struct SimpleLogger;
///
/// impl Reporter for SimpleLogger {
///     fn report(&self, metrics: &dyn MetricsProvider<'_>) -> Result<(), Box<dyn Error>> {
///         println!("Profiling {} complete", metrics.caller_name());
///         println!("Functions measured: {}", metrics.metric_data().len());
///         Ok(())
///     }
/// }
/// ```
///
/// # See Also
///
/// * [`MetricsProvider`] - Trait for accessing profiling metrics data
/// * [`GuardBuilder::reporter`](crate::GuardBuilder::reporter) - Method to set custom reporter
pub trait Reporter {
    fn report(
        &self,
        metrics_provider: &dyn MetricsProvider<'_>,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

/// Profiling mode indicating what type of measurements were collected.
///
/// This enum identifies which profiling feature was active when measurements
/// were collected. It's included in JSON output to help interpret the metrics.
///
/// # Variants
///
/// * `Timing` - Time-based profiling (execution duration)
/// * `AllocBytesTotal` - Total bytes allocated per function call
/// * `AllocBytesMax` - Peak memory usage per function call
/// * `AllocCountTotal` - Total allocation count per function call
/// * `AllocCountMax` - Peak allocation count per function call
#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum ProfilingMode {
    Timing,
    AllocBytesTotal,
    AllocBytesMax,
    AllocCountTotal,
    AllocCountMax,
}

/// JSON representation of profiling metrics.
#[derive(Serialize, Debug, Clone)]
pub struct MetricsJson {
    pub hotpath_profiling_mode: ProfilingMode,
    pub total_elapsed: u64,
    pub caller_name: String,
    pub output: MetricsDataJson,
}

#[derive(Deserialize)]
struct MetricsJsonRaw {
    hotpath_profiling_mode: ProfilingMode,
    total_elapsed: u64,
    caller_name: String,
    output: serde_json::Value,
}

impl TryFrom<MetricsJsonRaw> for MetricsJson {
    type Error = serde::de::value::Error;

    fn try_from(raw: MetricsJsonRaw) -> Result<Self, Self::Error> {
        let output =
            MetricsDataJson::deserialize_with_mode(raw.output, &raw.hotpath_profiling_mode)
                .map_err(serde::de::Error::custom)?;
        Ok(MetricsJson {
            hotpath_profiling_mode: raw.hotpath_profiling_mode,
            total_elapsed: raw.total_elapsed,
            caller_name: raw.caller_name,
            output,
        })
    }
}

impl<'de> Deserialize<'de> for MetricsJson {
    fn deserialize<D>(de: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = MetricsJsonRaw::deserialize(de)?;
        raw.try_into().map_err(serde::de::Error::custom)
    }
}

/// Structured per-function profiling metrics data.
#[derive(Debug, Clone)]
pub struct MetricsDataJson {
    pub headers: Vec<String>,
    pub function_names: Vec<String>,
    pub rows: Vec<Vec<MetricType>>,
}

impl Serialize for MetricsDataJson {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.rows.len()))?;

        for (i, row) in self.rows.iter().enumerate() {
            if i < self.function_names.len() {
                let function_name = &self.function_names[i];

                let function_serializer = FunctionDataSerializer {
                    headers: &self.headers,
                    row,
                };

                map.serialize_entry(function_name, &function_serializer)?;
            }
        }

        map.end()
    }
}

impl MetricsDataJson {
    pub fn deserialize_with_mode(
        value: serde_json::Value,
        profiling_mode: &ProfilingMode,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let map = value
            .as_object()
            .ok_or("Expected object for output field")?;

        let mut function_names = Vec::new();
        let mut rows = Vec::new();
        let mut headers = Vec::new();

        let mut first_entry = true;
        for (function_name, function_data) in map {
            function_names.push(function_name.clone());

            let function_obj = function_data
                .as_object()
                .ok_or("Expected object for function data")?;

            if first_entry {
                headers.push("Function".to_string());
                let mut metric_headers: Vec<String> = function_obj.keys().cloned().collect();
                metric_headers.sort();
                headers.extend(metric_headers.iter().cloned());
                first_entry = false;
            }

            let mut row = Vec::new();
            for header in headers.iter().skip(1) {
                if let Some(value) = function_obj.get(header) {
                    let value_u64 = value.as_u64().ok_or("Expected u64 value")?;
                    let metric_type = create_metric_type(header, value_u64, profiling_mode);
                    row.push(metric_type);
                }
            }
            rows.push(row);
        }

        Ok(MetricsDataJson {
            headers,
            function_names,
            rows,
        })
    }
}

fn create_metric_type(field_name: &str, value: u64, profiling_mode: &ProfilingMode) -> MetricType {
    match field_name {
        "calls" => MetricType::CallsCount(value),
        "percent_total" => MetricType::Percentage(value),
        // Percentiles
        name if name.starts_with('p') && name[1..].chars().all(|c| c.is_ascii_digit()) => {
            match profiling_mode {
                ProfilingMode::Timing => MetricType::DurationNs(value),
                ProfilingMode::AllocBytesTotal | ProfilingMode::AllocBytesMax => {
                    MetricType::AllocBytes(value)
                }
                ProfilingMode::AllocCountTotal | ProfilingMode::AllocCountMax => {
                    MetricType::AllocCount(value)
                }
            }
        }
        "avg" | "total" => match profiling_mode {
            ProfilingMode::Timing => MetricType::DurationNs(value),
            ProfilingMode::AllocBytesTotal | ProfilingMode::AllocBytesMax => {
                MetricType::AllocBytes(value)
            }
            ProfilingMode::AllocCountTotal | ProfilingMode::AllocCountMax => {
                MetricType::AllocCount(value)
            }
        },
        _ => unreachable!(),
    }
}

struct FunctionDataSerializer<'a> {
    headers: &'a [String],
    row: &'a [MetricType],
}

impl<'a> Serialize for FunctionDataSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.headers.len() - 1))?;

        for (i, header) in self.headers.iter().enumerate().skip(1) {
            if i - 1 < self.row.len() {
                let key = header
                    .to_lowercase()
                    .replace(' ', "_")
                    .replace('%', "percent");
                map.serialize_entry(&key, &self.row[i - 1])?;
            }
        }

        map.end()
    }
}

impl From<&dyn MetricsProvider<'_>> for MetricsJson {
    fn from(metrics: &dyn MetricsProvider<'_>) -> Self {
        let hotpath_profiling_mode = Self::determine_profiling_mode();

        let sorted_entries = get_sorted_entries(metrics);
        let (function_names, rows): (Vec<String>, Vec<Vec<MetricType>>) =
            sorted_entries.into_iter().unzip();

        Self {
            hotpath_profiling_mode,
            total_elapsed: metrics.total_elapsed(),
            caller_name: metrics.caller_name().to_string(),
            output: MetricsDataJson {
                headers: metrics.headers(),
                function_names,
                rows,
            },
        }
    }
}

impl MetricsJson {
    fn determine_profiling_mode() -> ProfilingMode {
        cfg_if::cfg_if! {
            if #[cfg(feature = "hotpath-alloc-bytes-total")] {
                ProfilingMode::AllocBytesTotal
            } else if #[cfg(feature = "hotpath-alloc-bytes-max")] {
                ProfilingMode::AllocBytesMax
            } else if #[cfg(feature = "hotpath-alloc-count-total")] {
                ProfilingMode::AllocCountTotal
            } else if #[cfg(feature = "hotpath-alloc-count-max")] {
                ProfilingMode::AllocCountMax
            } else {
                ProfilingMode::Timing
            }
        }
    }
}

pub(crate) fn display_table(metrics_provider: &dyn MetricsProvider<'_>) {
    let use_colors = std::env::var("NO_COLOR").is_err();

    let mut table = Table::new();

    let header_cells: Vec<Cell> = metrics_provider
        .headers()
        .into_iter()
        .map(|header| {
            if use_colors {
                Cell::new(&header)
                    .with_style(Attr::Bold)
                    .with_style(Attr::ForegroundColor(color::CYAN))
            } else {
                Cell::new(&header).with_style(Attr::Bold)
            }
        })
        .collect();

    table.add_row(Row::new(header_cells));

    let sorted_entries = get_sorted_entries(metrics_provider);

    for (function_name, metrics) in sorted_entries {
        let mut row_cells = Vec::new();

        row_cells.push(Cell::new(&function_name));

        for metric in &metrics {
            row_cells.push(Cell::new(&metric.to_string()));
        }

        table.add_row(Row::new(row_cells));
    }

    println!("{}", metrics_provider.description());
    table.printstd();

    if metrics_provider.has_unsupported_async() {
        println!();
        println!(
            "* {} for async methods is currently only available for tokio {} runtime.",
            "alloc profiling".yellow().bold(),
            "current_thread".green().bold()
        );
        println!(
            "  Please use {} to enable it.",
            "#[tokio::main(flavor = \"current_thread\")]".cyan().bold()
        );
    }
}

pub(crate) fn get_sorted_entries(
    metrics_provider: &dyn MetricsProvider<'_>,
) -> Vec<(String, Vec<MetricType>)> {
    let metric_data = metrics_provider.metric_data();

    let mut sorted_entries: Vec<(String, Vec<MetricType>)> = metric_data.into_iter().collect();
    sorted_entries.sort_by(|(_name_a, metrics_a), (_name_b, metrics_b)| {
        let key_a = metrics_provider.sort_key(metrics_a);
        let key_b = metrics_provider.sort_key(metrics_b);
        key_b
            .partial_cmp(&key_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    sorted_entries
}

/// Trait for accessing profiling metrics data from custom reporters.
///
/// This trait provides a standardized interface for reporters to access profiling
/// metrics, regardless of the underlying profiling mode (time or allocation tracking).
/// Implement [`Reporter`] to use this interface for custom output.
///
/// # Examples
///
/// ```rust
/// use hotpath::{Reporter, MetricsProvider};
/// use std::error::Error;
///
/// struct CustomReporter;
///
/// impl Reporter for CustomReporter {
///     fn report(&self, metrics: &dyn MetricsProvider<'_>) -> Result<(), Box<dyn Error>> {
///         println!("=== {} ===", metrics.description());
///
///         for (func_name, metric_values) in metrics.metric_data() {
///             println!("{}: {} values", func_name, metric_values.len());
///         }
///
///         Ok(())
///     }
/// }
/// ```
///
/// # See Also
///
/// * [`Reporter`] - Trait for implementing custom reporters
/// * [`MetricType`] - Metric value types
pub trait MetricsProvider<'a> {
    fn description(&self) -> String;
    fn headers(&self) -> Vec<String> {
        let mut headers = vec![
            "Function".to_string(),
            "Calls".to_string(),
            "Avg".to_string(),
        ];

        for &p in &self.percentiles() {
            headers.push(format!("P{}", p));
        }

        headers.push("Total".to_string());
        headers.push("% Total".to_string());

        headers
    }
    fn percentiles(&self) -> Vec<u8>;

    fn metric_data(&self) -> HashMap<String, Vec<MetricType>>;

    fn sort_key(&self, metrics: &[MetricType]) -> f64 {
        // Sort by percentage, higher percentages first
        if let Some(MetricType::Percentage(basis_points)) = metrics.last() {
            *basis_points as f64 / 100.0
        } else {
            0.0
        }
    }

    fn has_unsupported_async(&self) -> bool {
        false // Default implementation for time-based measurements
    }

    fn new(
        stats: &'a HashMap<&'static str, FunctionStats>,
        total_elapsed: Duration,
        percentiles: Vec<u8>,
        caller_name: String,
    ) -> Self
    where
        Self: Sized;

    fn total_elapsed(&self) -> u64;

    fn caller_name(&self) -> &str;
}

fn display_no_measurements_message(total_elapsed: Duration, caller_name: &str) {
    let title = format!(
        "\n{} No measurements recorded from {} (Total time: {:.2?})",
        "[hotpath]".blue().bold(),
        caller_name.yellow().bold(),
        total_elapsed
    );
    println!("{title}");
    println!();
    println!(
        "To start measuring performance, add the {} macro to your functions:",
        "#[hotpath::measure]".cyan().bold()
    );
    println!();
    println!(
        "  {}",
        "#[cfg_attr(feature = \"hotpath\", hotpath::measure)]".cyan()
    );
    println!("  {}", "fn your_function() {".dimmed());
    println!("  {}", "    // your code here".dimmed());
    println!("  {}", "}".dimmed());
    println!();
    println!(
        "Or use {} to measure code blocks:",
        "hotpath::measure_block!".cyan().bold()
    );
    println!();
    println!("  {}", "#[cfg(feature = \"hotpath\")]".cyan());
    println!("  {}", "hotpath::measure_block!(\"label\", {".cyan());
    println!("  {}", "    // your code here".dimmed());
    println!("  {}", "});".cyan());
    println!();
}

pub(crate) struct TableReporter;

impl Reporter for TableReporter {
    fn report(
        &self,
        metrics_provider: &dyn MetricsProvider<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if metrics_provider.metric_data().is_empty() {
            display_no_measurements_message(
                Duration::from_nanos(metrics_provider.total_elapsed()),
                metrics_provider.caller_name(),
            );
            return Ok(());
        }

        display_table(metrics_provider);
        Ok(())
    }
}

pub(crate) struct JsonReporter;

impl Reporter for JsonReporter {
    fn report(
        &self,
        metrics_provider: &dyn MetricsProvider<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if metrics_provider.metric_data().is_empty() {
            display_no_measurements_message(Duration::ZERO, metrics_provider.caller_name());
            return Ok(());
        }

        let json = MetricsJson::from(metrics_provider);
        println!("{}", serde_json::to_string(&json).unwrap());
        Ok(())
    }
}

pub(crate) struct JsonPrettyReporter;

impl Reporter for JsonPrettyReporter {
    fn report(
        &self,
        metrics_provider: &dyn MetricsProvider<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if metrics_provider.metric_data().is_empty() {
            display_no_measurements_message(Duration::ZERO, metrics_provider.caller_name());
            return Ok(());
        }

        let json = MetricsJson::from(metrics_provider);
        println!("{}", serde_json::to_string_pretty(&json)?);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_timing_mode() {
        let json_str = r#"{
            "hotpath_profiling_mode": "timing",
            "total_elapsed": 125189584,
            "caller_name": "basic::main",
            "output": {
                "basic::async_function": {
                    "calls": 100,
                    "avg": 1174672,
                    "p95": 1201151,
                    "total": 117467210,
                    "percent_total": 9383
                },
                "basic::sync_function": {
                    "calls": 100,
                    "avg": 22563,
                    "p95": 33887,
                    "total": 2256381,
                    "percent_total": 180
                },
                "custom_block": {
                    "calls": 100,
                    "avg": 21936,
                    "p95": 33087,
                    "total": 2193628,
                    "percent_total": 175
                }
            }
        }"#;

        let metrics: MetricsJson =
            serde_json::from_str(json_str).expect("Failed to deserialize timing mode JSON");

        assert!(matches!(
            metrics.hotpath_profiling_mode,
            ProfilingMode::Timing
        ));
        assert_eq!(metrics.total_elapsed, 125189584);
        assert_eq!(metrics.caller_name, "basic::main");
        assert_eq!(metrics.output.function_names.len(), 3);
        assert!(metrics
            .output
            .function_names
            .contains(&"basic::async_function".to_string()));
        assert!(metrics
            .output
            .function_names
            .contains(&"basic::sync_function".to_string()));
        assert!(metrics
            .output
            .function_names
            .contains(&"custom_block".to_string()));

        // Verify that timing mode creates Timing MetricTypes for avg, p95, total
        let first_row = &metrics.output.rows[0];
        assert!(matches!(first_row[0], MetricType::DurationNs(_))); // avg
        assert!(matches!(first_row[1], MetricType::CallsCount(_))); // calls
        assert!(matches!(first_row[2], MetricType::DurationNs(_))); // p95
        assert!(matches!(first_row[3], MetricType::Percentage(_))); // percent_total
        assert!(matches!(first_row[4], MetricType::DurationNs(_))); // total
    }

    #[test]
    fn test_deserialize_alloc_count_max_mode() {
        let json_str = r#"{
            "hotpath_profiling_mode": "alloc-count-max",
            "total_elapsed": 123848875,
            "caller_name": "basic::main",
            "output": {
                "custom_block": {
                    "calls": 100,
                    "avg": 2,
                    "p95": 2,
                    "total": 200,
                    "percent_total": 5000
                },
                "basic::sync_function": {
                    "calls": 100,
                    "avg": 1,
                    "p95": 1,
                    "total": 100,
                    "percent_total": 2500
                },
                "basic::async_function": {
                    "calls": 100,
                    "avg": 1,
                    "p95": 1,
                    "total": 100,
                    "percent_total": 2500
                }
            }
        }"#;

        let metrics: MetricsJson = serde_json::from_str(json_str)
            .expect("Failed to deserialize alloc-count-max mode JSON");

        assert!(matches!(
            metrics.hotpath_profiling_mode,
            ProfilingMode::AllocCountMax
        ));
        assert_eq!(metrics.total_elapsed, 123848875);
        assert_eq!(metrics.caller_name, "basic::main");
        assert_eq!(metrics.output.function_names.len(), 3);
        assert!(metrics
            .output
            .function_names
            .contains(&"custom_block".to_string()));
        assert!(metrics
            .output
            .function_names
            .contains(&"basic::sync_function".to_string()));
        assert!(metrics
            .output
            .function_names
            .contains(&"basic::async_function".to_string()));

        let first_row = &metrics.output.rows[0];
        assert!(matches!(first_row[0], MetricType::AllocCount(_))); // avg
        assert!(matches!(first_row[1], MetricType::CallsCount(_))); // calls
        assert!(matches!(first_row[2], MetricType::AllocCount(_))); // p95
        assert!(matches!(first_row[3], MetricType::Percentage(_))); // percent_total
        assert!(matches!(first_row[4], MetricType::AllocCount(_))); // total
    }

    #[test]
    fn test_deserialize_alloc_count_total_mode() {
        let json_str = r#"{
            "hotpath_profiling_mode": "alloc-count-total",
            "total_elapsed": 123762083,
            "caller_name": "basic::main",
            "output": {
                "basic::sync_function": {
                    "calls": 100,
                    "avg": 2,
                    "p95": 2,
                    "total": 200,
                    "percent_total": 3333
                },
                "basic::async_function": {
                    "calls": 100,
                    "avg": 2,
                    "p95": 2,
                    "total": 200,
                    "percent_total": 3333
                },
                "custom_block": {
                    "calls": 100,
                    "avg": 2,
                    "p95": 2,
                    "total": 200,
                    "percent_total": 3333
                }
            }
        }"#;

        let metrics: MetricsJson = serde_json::from_str(json_str)
            .expect("Failed to deserialize alloc-count-total mode JSON");

        assert!(matches!(
            metrics.hotpath_profiling_mode,
            ProfilingMode::AllocCountTotal
        ));
        assert_eq!(metrics.total_elapsed, 123762083);
        assert_eq!(metrics.caller_name, "basic::main");
        assert_eq!(metrics.output.function_names.len(), 3);

        let first_row = &metrics.output.rows[0];
        assert!(matches!(first_row[0], MetricType::AllocCount(_))); // avg
        assert!(matches!(first_row[1], MetricType::CallsCount(_))); // calls
        assert!(matches!(first_row[2], MetricType::AllocCount(_))); // p95
        assert!(matches!(first_row[3], MetricType::Percentage(_))); // percent_total
        assert!(matches!(first_row[4], MetricType::AllocCount(_))); // total
    }

    #[test]
    fn test_deserialize_alloc_bytes_max_mode() {
        let json_str = r#"{
            "hotpath_profiling_mode": "alloc-bytes-max",
            "total_elapsed": 119932458,
            "caller_name": "basic::main",
            "output": {
                "custom_block": {
                    "calls": 100,
                    "avg": 1088,
                    "p95": 1088,
                    "total": 108800,
                    "percent_total": 9066
                },
                "basic::sync_function": {
                    "calls": 100,
                    "avg": 76,
                    "p95": 76,
                    "total": 7600,
                    "percent_total": 633
                },
                "basic::async_function": {
                    "calls": 100,
                    "avg": 36,
                    "p95": 36,
                    "total": 3600,
                    "percent_total": 300
                }
            }
        }"#;

        let metrics: MetricsJson = serde_json::from_str(json_str)
            .expect("Failed to deserialize alloc-bytes-max mode JSON");

        assert!(matches!(
            metrics.hotpath_profiling_mode,
            ProfilingMode::AllocBytesMax
        ));
        assert_eq!(metrics.total_elapsed, 119932458);
        assert_eq!(metrics.caller_name, "basic::main");
        assert_eq!(metrics.output.function_names.len(), 3);

        let first_row = &metrics.output.rows[0];
        assert!(matches!(first_row[0], MetricType::AllocBytes(_))); // avg
        assert!(matches!(first_row[1], MetricType::CallsCount(_))); // calls
        assert!(matches!(first_row[2], MetricType::AllocBytes(_))); // p95
        assert!(matches!(first_row[3], MetricType::Percentage(_))); // percent_total
        assert!(matches!(first_row[4], MetricType::AllocBytes(_))); // total
    }

    #[test]
    fn test_deserialize_alloc_bytes_total_mode() {
        let json_str = r#"{
            "hotpath_profiling_mode": "alloc-bytes-total",
            "total_elapsed": 121738041,
            "caller_name": "basic::main",
            "output": {
                "custom_block": {
                    "calls": 100,
                    "avg": 1088,
                    "p95": 1088,
                    "total": 108800,
                    "percent_total": 8292
                },
                "basic::sync_function": {
                    "calls": 100,
                    "avg": 152,
                    "p95": 152,
                    "total": 15200,
                    "percent_total": 1158
                },
                "basic::async_function": {
                    "calls": 100,
                    "avg": 72,
                    "p95": 72,
                    "total": 7200,
                    "percent_total": 548
                }
            }
        }"#;

        let metrics: MetricsJson = serde_json::from_str(json_str)
            .expect("Failed to deserialize alloc-bytes-total mode JSON");

        assert!(matches!(
            metrics.hotpath_profiling_mode,
            ProfilingMode::AllocBytesTotal
        ));
        assert_eq!(metrics.total_elapsed, 121738041);
        assert_eq!(metrics.caller_name, "basic::main");
        assert_eq!(metrics.output.function_names.len(), 3);

        let first_row = &metrics.output.rows[0];
        assert!(matches!(first_row[0], MetricType::AllocBytes(_))); // avg
        assert!(matches!(first_row[1], MetricType::CallsCount(_))); // calls
        assert!(matches!(first_row[2], MetricType::AllocBytes(_))); // p95
        assert!(matches!(first_row[3], MetricType::Percentage(_))); // percent_total
        assert!(matches!(first_row[4], MetricType::AllocBytes(_))); // total
    }

    use serde_json::Value;

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let original_json_str = r#"{
            "hotpath_profiling_mode": "timing",
            "total_elapsed": 125189584,
            "caller_name": "basic::main",
            "output": {
                "basic::async_function": {
                    "calls": 100,
                    "avg": 1174672,
                    "p95": 1201151,
                    "total": 117467210,
                    "percent_total": 9383
                }
            }
        }"#;

        let metrics: MetricsJson =
            serde_json::from_str(original_json_str).expect("Failed to deserialize");
        let serialized_str = serde_json::to_string(&metrics).expect("Failed to serialize");

        let original_json: Value = serde_json::from_str(original_json_str).unwrap();
        let serialized_json: Value = serde_json::from_str(&serialized_str).unwrap();
        assert_eq!(serialized_json, original_json);
    }

    #[test]
    fn test_metric_data_structure() {
        let json_str = r#"{
            "hotpath_profiling_mode": "timing",
            "total_elapsed": 125189584,
            "caller_name": "basic::main",
            "output": {
                "test_function": {
                    "calls": 42,
                    "avg": 1000,
                    "p95": 2000,
                    "total": 42000,
                    "percent_total": 100
                }
            }
        }"#;

        let metrics: MetricsJson = serde_json::from_str(json_str).expect("Failed to deserialize");

        // Verify that the internal structure is correctly parsed
        assert_eq!(metrics.output.headers.len(), 6); // Function, calls, avg, p95, total, percent_total
        assert_eq!(metrics.output.headers[0], "Function");
        assert!(metrics.output.headers.contains(&"calls".to_string()));
        assert!(metrics.output.headers.contains(&"avg".to_string()));
        assert!(metrics.output.headers.contains(&"p95".to_string()));
        assert!(metrics.output.headers.contains(&"total".to_string()));
        assert!(metrics
            .output
            .headers
            .contains(&"percent_total".to_string()));

        assert_eq!(metrics.output.function_names.len(), 1);
        assert_eq!(metrics.output.function_names[0], "test_function");

        assert_eq!(metrics.output.rows.len(), 1);
        assert_eq!(metrics.output.rows[0].len(), 5); // All metrics except function name
    }
}
