use super::FunctionStats;
use colored::*;
use prettytable::{color, Attr, Cell, Row, Table};
use serde::{
    ser::{SerializeMap, Serializer},
    Serialize,
};
use std::collections::HashMap;
use std::fmt;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum MetricType {
    CallsCount(u64), // Number of function calls
    Timing(u64),     // Duration in nanoseconds
    AllocBytes(u64), // Bytes allocated
    AllocCount(u64), // Allocation count
    Percentage(u64), // Percentage as basis points (1% = 100)
    Unsupported,     // For N/A values (async functions when not supported)
}

impl fmt::Display for MetricType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetricType::CallsCount(count) => {
                write!(f, "{}", count)
            }
            MetricType::Timing(ns) => {
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

impl Serialize for MetricType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            MetricType::CallsCount(count) => serializer.serialize_u64(*count),
            MetricType::Timing(ns) => serializer.serialize_u64(*ns),
            MetricType::AllocBytes(bytes) => serializer.serialize_u64(*bytes),
            MetricType::AllocCount(count) => serializer.serialize_u64(*count),
            MetricType::Percentage(basis_points) => serializer.serialize_u64(*basis_points),
            MetricType::Unsupported => serializer.serialize_none(),
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

// Helper function to format function names consistently
pub fn format_function_name(function_name: &str) -> String {
    let parts: Vec<&str> = function_name.split("::").collect();
    if parts.len() > 2 {
        parts[parts.len() - 2..].join("::")
    } else {
        function_name.to_string()
    }
}

// Helper function to get sorted entries from metric data
pub(crate) fn get_sorted_entries<'a, T: MetricsProvider<'a>>(
    tableable: &T,
) -> Vec<(String, Vec<MetricType>)> {
    let metric_data = tableable.metric_data();

    let mut sorted_entries: Vec<(String, Vec<MetricType>)> = metric_data.into_iter().collect();
    sorted_entries.sort_by(|(name_a, metrics_a), (name_b, metrics_b)| {
        let key_a = tableable.sort_key(name_a, metrics_a);
        let key_b = tableable.sort_key(name_b, metrics_b);
        key_b
            .partial_cmp(&key_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    sorted_entries
}

pub trait Reporter {
    fn report(&self, metrics_provider: &dyn MetricsProvider<'_>, caller_name: &str);
}

#[allow(dead_code)]
#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfilingMode {
    Timing,
    AllocBytesTotal,
    AllocBytesMax,
    AllocCountTotal,
    AllocCountMax,
}

#[derive(Serialize)]
pub struct SerializableOutput {
    pub hotpath_profiling_mode: ProfilingMode,
    pub output: SerializableTable,
}

pub struct SerializableTable {
    pub headers: Vec<String>,
    pub function_names: Vec<String>,
    pub rows: Vec<Vec<MetricType>>,
}

impl Serialize for SerializableTable {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.rows.len()))?;

        for (i, row) in self.rows.iter().enumerate() {
            if i < self.function_names.len() {
                let function_name = &self.function_names[i];

                // Create ordered function data using a nested map serializer
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

        // Skip the first header (Function) and iterate in order
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

impl<'a, T> From<(&T, &str)> for SerializableOutput
where
    T: MetricsProvider<'a>,
{
    fn from((tableable, _caller_name): (&T, &str)) -> Self {
        let hotpath_profiling_mode = Self::determine_profiling_mode(&tableable.headers());

        // Use new interface: get sorted entries and separate into names and rows
        let sorted_entries = get_sorted_entries(tableable);
        let (function_names, rows): (Vec<String>, Vec<Vec<MetricType>>) =
            sorted_entries.into_iter().unzip();

        Self {
            hotpath_profiling_mode,
            output: SerializableTable {
                headers: tableable.headers(),
                function_names,
                rows,
            },
        }
    }
}

impl SerializableOutput {
    fn determine_profiling_mode(_headers: &[String]) -> ProfilingMode {
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

pub(crate) trait MetricsProviderSerialize<'a>: MetricsProvider<'a> {
    fn to_serializable_table(&self, caller_name: &str) -> SerializableOutput
    where
        Self: Sized,
    {
        SerializableOutput::from((self, caller_name))
    }
}

impl<'a, T> MetricsProviderSerialize<'a> for T where T: MetricsProvider<'a> {}

// Helper functions for working with trait objects
pub(crate) fn display_table_dyn(metrics_provider: &dyn MetricsProvider<'_>, caller_name: &str) {
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

    // Use new interface: get sorted entries and separate into names and rows
    let sorted_entries = get_sorted_entries_dyn(metrics_provider);

    for (function_name, metrics) in sorted_entries {
        let mut row_cells = Vec::new();

        // Add function name as first cell
        row_cells.push(Cell::new(&function_name));

        // Add metric data cells, using Display implementation
        for metric in &metrics {
            row_cells.push(Cell::new(&metric.to_string()));
        }

        table.add_row(Row::new(row_cells));
    }

    println!("{}", metrics_provider.description(caller_name));
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

pub(crate) fn get_sorted_entries_dyn(
    metrics_provider: &dyn MetricsProvider<'_>,
) -> Vec<(String, Vec<MetricType>)> {
    let metric_data = metrics_provider.metric_data();

    let mut sorted_entries: Vec<(String, Vec<MetricType>)> = metric_data.into_iter().collect();
    sorted_entries.sort_by(|(name_a, metrics_a), (name_b, metrics_b)| {
        let key_a = metrics_provider.sort_key(name_a, metrics_a);
        let key_b = metrics_provider.sort_key(name_b, metrics_b);
        key_b
            .partial_cmp(&key_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    sorted_entries
}

// Helper function to create SerializableOutput from trait object
pub(crate) fn create_serializable_output(
    metrics_provider: &dyn MetricsProvider<'_>,
    _caller_name: &str,
) -> SerializableOutput {
    let hotpath_profiling_mode = SerializableOutput::determine_profiling_mode(&metrics_provider.headers());

    // Use new interface: get sorted entries and separate into names and rows
    let sorted_entries = get_sorted_entries_dyn(metrics_provider);
    let (function_names, rows): (Vec<String>, Vec<Vec<MetricType>>) =
        sorted_entries.into_iter().unzip();

    SerializableOutput {
        hotpath_profiling_mode,
        output: SerializableTable {
            headers: metrics_provider.headers(),
            function_names,
            rows,
        },
    }
}

pub(crate) fn display_table<'a, T: MetricsProvider<'a>>(tableable: T, caller_name: &str) {
    let use_colors = std::env::var("NO_COLOR").is_err();

    let mut table = Table::new();

    let header_cells: Vec<Cell> = tableable
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

    // Use new interface: get sorted entries and separate into names and rows
    let sorted_entries = get_sorted_entries(&tableable);

    for (function_name, metrics) in sorted_entries {
        let mut row_cells = Vec::new();

        // Add function name as first cell
        row_cells.push(Cell::new(&function_name));

        // Add metric data cells, using Display implementation
        for metric in &metrics {
            row_cells.push(Cell::new(&metric.to_string()));
        }

        table.add_row(Row::new(row_cells));
    }

    println!("{}", tableable.description(caller_name));
    table.printstd();

    if tableable.has_unsupported_async() {
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

pub trait MetricsProvider<'a> {
    fn description(&self, caller_name: &str) -> String;
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

    fn sort_key(&self, _function_name: &str, metrics: &[MetricType]) -> f64 {
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
}

pub fn display_no_measurements_message(total_elapsed: Duration, caller_name: &str) {
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

pub struct TableReporter;

impl Reporter for TableReporter {
    fn report(&self, metrics_provider: &dyn MetricsProvider<'_>, caller_name: &str) {
        if metrics_provider.metric_data().is_empty() {
            // We don't have direct access to total_elapsed anymore, so use Duration::ZERO
            display_no_measurements_message(Duration::ZERO, caller_name);
            return;
        }

        display_table_dyn(metrics_provider, caller_name);
    }
}

pub struct JsonReporter;

impl Reporter for JsonReporter {
    fn report(&self, metrics_provider: &dyn MetricsProvider<'_>, caller_name: &str) {
        if metrics_provider.metric_data().is_empty() {
            display_no_measurements_message(Duration::ZERO, caller_name);
            return;
        }

        let json = create_serializable_output(metrics_provider, caller_name);
        println!("{}", serde_json::to_string(&json).unwrap());
    }
}

pub struct JsonPrettyReporter;

impl Reporter for JsonPrettyReporter {
    fn report(&self, metrics_provider: &dyn MetricsProvider<'_>, caller_name: &str) {
        if metrics_provider.metric_data().is_empty() {
            display_no_measurements_message(Duration::ZERO, caller_name);
            return;
        }

        let json = create_serializable_output(metrics_provider, caller_name);
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
    }
}
