pub use cfg_if::cfg_if;
pub use hotpath_macros::{main, measure};
pub use output::Reporter;

cfg_if::cfg_if! {
    if #[cfg(any(
        feature = "hotpath-alloc-bytes-total",
        feature = "hotpath-alloc-bytes-max",
        feature = "hotpath-alloc-count-total",
        feature = "hotpath-alloc-count-max"
    ))] {
        mod alloc;
        pub use tokio::runtime::{Handle, RuntimeFlavor};

        // Memory allocations profiling using a custom global allocator
        #[global_allocator]
        static GLOBAL: alloc::allocator::CountingAllocator = alloc::allocator::CountingAllocator {};
        pub use alloc::shared::NoopAsyncAllocGuard;

        pub enum AllocGuardType {
            AllocGuard(AllocGuard),
            NoopAsyncAllocGuard(NoopAsyncAllocGuard),
        }
    } else {
        // Time-based profiling (when no allocation features are enabled)
        mod time;
        pub use time::guard::TimeGuard;
        pub use time::state::FunctionStats;
        use time::{
            report::StatsTable,
            state::{HotPathState, Measurement, process_measurement},
        };
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "hotpath-alloc-bytes-max")] {
        mod alloc_bytes_max;
        pub use alloc_bytes_max::{core::AllocationInfo, guard::AllocGuard};
        pub use alloc_bytes_max::state::FunctionStats;
        use alloc_bytes_max::{
            report::StatsTable,
            state::{HotPathState, Measurement, process_measurement},
        };
    } else if #[cfg(feature = "hotpath-alloc-bytes-total")] {
        mod alloc_bytes_total;
        pub use alloc_bytes_total::{core::AllocationInfo, guard::AllocGuard};
        pub use alloc_bytes_total::state::FunctionStats;
        use alloc_bytes_total::{
            report::StatsTable,
            state::{HotPathState, Measurement, process_measurement},
        };
    } else if #[cfg(feature = "hotpath-alloc-count-max")] {
        mod alloc_count_max;
        pub use alloc_count_max::{core::AllocationInfo, guard::AllocGuard};
        pub use alloc_count_max::state::FunctionStats;
        use alloc_count_max::{
            report::StatsTable,
            state::{HotPathState, Measurement, process_measurement},
        };
    } else if #[cfg(feature = "hotpath-alloc-count-total")] {
        mod alloc_count_total;
        pub use alloc_count_total::{core::AllocationInfo, guard::AllocGuard};
        pub use alloc_count_total::state::FunctionStats;
        use alloc_count_total::{
            report::StatsTable,
            state::{HotPathState, Measurement, process_measurement},
        };
    }
}

pub mod output;

#[derive(Clone, Copy, Debug, Default)]
pub enum Format {
    #[default]
    Table,
    Json,
    JsonPretty,
}

use crossbeam_channel::{bounded, select};
use std::collections::HashMap;
use std::thread;
use std::time::Instant;

#[cfg(feature = "hotpath")]
#[macro_export]
macro_rules! measure_block {
    ($label:expr, $expr:expr) => {{
        hotpath::cfg_if! {
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
    ($label:expr, $expr:expr) => {{
        $expr
    }};
}

use arc_swap::ArcSwapOption;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::RwLock;

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

pub static HOTPATH_STATE: OnceLock<ArcSwapOption<RwLock<HotPathState>>> = OnceLock::new();

pub fn init(caller_name: String, percentiles: &[u8], format: Format) -> HotPath {
    let percentiles = percentiles.to_vec();

    let arc_swap = HOTPATH_STATE.get_or_init(|| ArcSwapOption::from(None));

    if arc_swap.load().is_some() {
        panic!("More than one _hotpath guard cannot be alive at the same time.");
    }

    let (tx, rx) = bounded::<Measurement>(10000);
    let (shutdown_tx, shutdown_rx) = bounded::<()>(1);
    let (completion_tx, completion_rx) = bounded::<HashMap<&'static str, FunctionStats>>(1);
    let start_time = Instant::now();

    let state_arc = Arc::new(RwLock::new(HotPathState {
        sender: Some(tx),
        shutdown_tx: Some(shutdown_tx),
        completion_rx: Some(completion_rx),
        start_time,
        caller_name,
        percentiles,
        format,
    }));

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

            // Send stats via completion channel
            let _ = completion_tx.send(local_stats);
        })
        .expect("Failed to spawn hotpath-worker thread");

    arc_swap.store(Some(Arc::clone(&state_arc)));

    let reporter: Box<dyn Reporter> = match format {
        Format::Table => Box::new(output::TableReporter),
        Format::Json => Box::new(output::JsonReporter),
        Format::JsonPretty => Box::new(output::JsonPrettyReporter),
    };

    HotPath {
        state: Arc::clone(&state_arc),
        reporter,
    }
}
pub struct HotPath {
    state: Arc<RwLock<HotPathState>>,
    reporter: Box<dyn Reporter>,
}

impl HotPath {
    pub fn set_reporter(&mut self, reporter: Box<dyn Reporter>) {
        self.reporter = reporter;
    }
}

impl Drop for HotPath {
    fn drop(&mut self) {
        let state: Arc<RwLock<HotPathState>> = Arc::clone(&self.state);

        // Signal shutdown and wait for processing thread to complete
        let (shutdown_tx, completion_rx, end_time) = {
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
            if let Ok(stats) = rx.recv() {
                if let Ok(state_guard) = state.read() {
                    let total_elapsed = end_time.duration_since(state_guard.start_time);
                    self.reporter.report(
                        &stats,
                        total_elapsed,
                        &state_guard.caller_name,
                        &state_guard.percentiles,
                    );
                }
            }
        }

        if let Some(arc_swap) = HOTPATH_STATE.get() {
            arc_swap.store(None);
        }
    }
}
