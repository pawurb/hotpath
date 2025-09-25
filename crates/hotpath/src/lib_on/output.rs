cfg_if::cfg_if! {
    if #[cfg(any(
        feature = "hotpath-alloc-bytes-total",
        feature = "hotpath-alloc-bytes-max",
        feature = "hotpath-alloc-count-total",
        feature = "hotpath-alloc-count-max"
    ))] {
        use super::{FunctionStats, StatsTable};
    } else {
        use super::{FunctionStats, StatsTable};
    }
}
use colored::*;
use prettytable::{color, Attr, Cell, Row, Table};
use serde::{
    ser::{SerializeMap, Serializer},
    Serialize,
};
use std::collections::HashMap;
use std::time::Duration;

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
    pub rows: Vec<Vec<String>>,
}

impl Serialize for SerializableTable {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.rows.len()))?;

        for row in &self.rows {
            if !row.is_empty() {
                let function_name = &row[0];

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
    row: &'a [String],
}

impl<'a> Serialize for FunctionDataSerializer<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.headers.len() - 1))?;

        // Skip the first header (Function) and iterate in order
        for (i, header) in self.headers.iter().enumerate().skip(1) {
            if i < self.row.len() {
                let key = header
                    .to_lowercase()
                    .replace(' ', "_")
                    .replace('%', "percent");
                map.serialize_entry(&key, &self.row[i])?;
            }
        }

        map.end()
    }
}

impl<'a, T> From<(&T, &str)> for SerializableOutput
where
    T: Tableable<'a>,
{
    fn from((tableable, _caller_name): (&T, &str)) -> Self {
        let hotpath_profiling_mode = Self::determine_profiling_mode(&tableable.headers());

        Self {
            hotpath_profiling_mode,
            output: SerializableTable {
                headers: tableable.headers(),
                rows: tableable.rows(),
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

pub(crate) trait TableableSerialize<'a>: Tableable<'a> {
    fn to_serializable_table(&self, caller_name: &str) -> SerializableOutput
    where
        Self: Sized,
    {
        SerializableOutput::from((self, caller_name))
    }
}

impl<'a, T> TableableSerialize<'a> for T where T: Tableable<'a> {}

pub(crate) fn display_table<'a, T: Tableable<'a>>(tableable: T, caller_name: &str) {
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

    for row_data in tableable.rows() {
        let row_cells: Vec<Cell> = row_data
            .into_iter()
            .map(|cell_data| Cell::new(&cell_data))
            .collect();
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

pub(crate) trait Tableable<'a> {
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
    fn rows(&self) -> Vec<Vec<String>>;
    fn has_unsupported_async(&self) -> bool {
        false // Default implementation for time-based measurements
    }
    fn new(
        stats: &'a HashMap<&'static str, FunctionStats>,
        total_elapsed: Duration,
        percentiles: Vec<u8>,
    ) -> Self;
}

pub fn display_performance_summary(
    stats: &HashMap<&'static str, FunctionStats>,
    total_elapsed: Duration,
    caller_name: &str,
    percentiles: &[u8],
    format: super::Format,
) {
    let has_data = stats.values().any(|s| s.has_data);

    if !has_data {
        println!("\nNo measurement data available.");
        return;
    }

    match format {
        super::Format::Table => {
            display_table(
                StatsTable::new(stats, total_elapsed, percentiles.to_vec()),
                caller_name,
            );
        }
        super::Format::Json => {
            let json = StatsTable::new(stats, total_elapsed, percentiles.to_vec())
                .to_serializable_table(caller_name);
            println!("{}", serde_json::to_string(&json).unwrap());
        }
        super::Format::JsonPretty => {
            let json = StatsTable::new(stats, total_elapsed, percentiles.to_vec())
                .to_serializable_table(caller_name);
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
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
