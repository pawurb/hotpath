pub use hotpath_macros::{main, measure};

cfg_if::cfg_if! {
    if #[cfg(any(
        feature = "hotpath-alloc-bytes-total",
        feature = "hotpath-alloc-bytes-max",
        feature = "hotpath-alloc-count-total",
        feature = "hotpath-alloc-count-max"
    ))] {
        pub mod alloc;

        // Shared global allocator
        #[global_allocator]
        static GLOBAL: alloc::allocator::CountingAllocator = alloc::allocator::CountingAllocator {};
    } else {
        // Time-based profiling (when no allocation features are enabled)
        pub mod time;
        pub use time::guard::TimeGuard;
        pub use time::state::send_duration_measurement;
        use crate::time::{
            report::StatsTable,
            state::{FunctionStats, HotPathState, Measurement, process_measurement},
        };
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "hotpath-alloc-bytes-max")] {
        pub mod alloc_bytes_max;
        pub use alloc_bytes_max::{core::AllocationInfo, guard::AllocGuard};
        use crate::alloc_bytes_max::{
            report::StatsTable,
            state::{FunctionStats, HotPathState, Measurement, process_measurement},
        };
    } else if #[cfg(feature = "hotpath-alloc-bytes-total")] {
        pub mod alloc_bytes_total;
        pub use alloc_bytes_total::{core::AllocationInfo, guard::AllocGuard};
        use crate::alloc_bytes_total::{
            report::StatsTable,
            state::{FunctionStats, HotPathState, Measurement, process_measurement},
        };
    } else if #[cfg(feature = "hotpath-alloc-count-max")] {
        pub mod alloc_count_max;
        pub use alloc_count_max::{core::AllocationInfo, guard::AllocGuard};
        use crate::alloc_count_max::{
            report::StatsTable,
            state::{FunctionStats, HotPathState, Measurement, process_measurement},
        };
    } else if #[cfg(feature = "hotpath-alloc-count-total")] {
        pub mod alloc_count_total;
        pub use alloc_count_total::{core::AllocationInfo, guard::AllocGuard};
        use crate::alloc_count_total::{
            report::StatsTable,
            state::{FunctionStats, HotPathState, Measurement, process_measurement},
        };
    }
}

use std::time::Duration;

use colored::*;
use crossbeam_channel::{bounded, select};
use std::collections::HashMap;
use std::thread;
use std::time::Instant;

#[cfg(feature = "hotpath")]
#[macro_export]
macro_rules! measure_block {
    ($label:expr, $expr:expr) => {{
        cfg_if::cfg_if! {
            if #[cfg(any(
                feature = "hotpath-alloc-bytes-total",
                feature = "hotpath-alloc-bytes-max",
                feature = "hotpath-alloc-count-total",
                feature = "hotpath-alloc-count-max"
            ))] {
                let _guard = hotpath::AllocGuard::new($label);
            } else {
                let _guard = hotpath::TimeGuard::new($label);
            }
        }

        $expr
    }};
}

#[cfg(not(feature = "hotpath"))]
#[macro_export]
macro_rules! measure_block {
    ($label:expr, $expr:expr) => {{ $expr }};
}

use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::RwLock;

// Compile-time check: ensure only one allocation feature is enabled at a time
#[cfg(all(
    feature = "hotpath-alloc-bytes-total",
    any(
        feature = "hotpath-alloc-bytes-max",
        feature = "hotpath-alloc-count-total",
        feature = "hotpath-alloc-count-max"
    )
))]
compile_error!("Only one allocation feature can be enabled at a time");

#[cfg(all(
    feature = "hotpath-alloc-bytes-max",
    any(
        feature = "hotpath-alloc-count-total",
        feature = "hotpath-alloc-count-max"
    )
))]
compile_error!("Only one allocation feature can be enabled at a time");

#[cfg(all(
    feature = "hotpath-alloc-count-total",
    feature = "hotpath-alloc-count-max"
))]
compile_error!("Only one allocation feature can be enabled at a time");

static HOTPATH_STATE: OnceLock<Arc<RwLock<HotPathState>>> = OnceLock::new();

pub fn init(caller_name: String, percentiles: &[u8]) -> HotPath {
    if HOTPATH_STATE.get().is_some() {
        panic!("hotpath::init() can be called only once");
    }

    let percentiles = percentiles.to_vec();

    let state = HOTPATH_STATE.get_or_init(|| {
        let (tx, rx) = bounded::<Measurement>(10000);
        let (shutdown_tx, shutdown_rx) = bounded::<()>(1);
        let (completion_tx, completion_rx) = bounded::<()>(1);
        let start_time = Instant::now();

        let state_arc = Arc::new(RwLock::new(HotPathState {
            sender: Some(tx),
            shutdown_tx: Some(shutdown_tx),
            completion_rx: Some(completion_rx),
            stats: None, // Will be populated by worker at shutdown
            start_time,
            caller_name,
            percentiles,
        }));

        let state_clone = Arc::clone(&state_arc);
        thread::Builder::new()
            .name("hotpath-worker".into())
            .spawn(move || {
                let mut local_stats = HashMap::<&'static str, FunctionStats>::new();

                loop {
                    select! {
                        recv(rx) -> result => {
                            match result {
                                Ok(measurement) => {
                                    process_measurement(&mut local_stats, measurement);
                                }
                                Err(_) => break, // Channel disconnected
                            }
                        }
                        recv(shutdown_rx) -> _ => {
                            // Process remaining messages after shutdown signal
                            while let Ok(measurement) = rx.try_recv() {
                                process_measurement(&mut local_stats, measurement);
                            }
                            break;
                        }
                    }
                }

                // Copy stats back to shared state before signaling completion
                if let Ok(mut state_guard) = state_clone.write() {
                    state_guard.stats = Some(local_stats);
                }

                let _ = completion_tx.send(());
            })
            .expect("Failed to spawn hotpath-worker thread");

        state_arc
    });

    HotPath {
        state: Arc::clone(state),
    }
}

pub struct HotPath {
    state: Arc<RwLock<HotPathState>>,
}

impl Drop for HotPath {
    fn drop(&mut self) {
        let state = Arc::clone(&self.state);

        // Signal shutdown and wait for processing thread to complete
        let (shutdown_tx, completion_rx, _end_time) = {
            let Ok(mut state_guard) = state.write() else {
                return;
            };

            state_guard.sender = None;
            let end_time = Instant::now();

            let shutdown_tx = state_guard.shutdown_tx.take();
            let completion_rx = state_guard.completion_rx.take();
            (shutdown_tx, completion_rx, end_time)
        };

        if let Some(tx) = shutdown_tx {
            let _ = tx.send(());
        }

        if let Some(rx) = completion_rx {
            let _ = rx.recv();
        }

        if let Ok(state_guard) = state.read()
            && let Some(ref stats) = state_guard.stats
        {
            let total_elapsed = _end_time.duration_since(state_guard.start_time);
            if stats.is_empty() {
                display_no_measurements_message(total_elapsed, &state_guard.caller_name);
            } else {
                display_performance_summary(
                    stats,
                    total_elapsed,
                    &state_guard.caller_name,
                    &state_guard.percentiles,
                );
            }
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

pub fn display_performance_summary(
    stats: &HashMap<&'static str, FunctionStats>,
    total_elapsed: Duration,
    caller_name: &str,
    percentiles: &[u8],
) {
    let has_data = stats.values().any(|s| s.has_data);

    if has_data {
        display_table(
            StatsTable::new(stats, total_elapsed, percentiles.to_vec()),
            caller_name,
        );
    } else {
        println!("\nNo measurement data available.");
    }
}

use prettytable::{Attr, Cell, Row, Table, color};

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
    fn new(
        stats: &'a HashMap<&'static str, FunctionStats>,
        total_elapsed: Duration,
        percentiles: Vec<u8>,
    ) -> Self;
}

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
}
