pub use hotpath_macros::measure;

use crossbeam_channel::{Receiver, Sender, bounded, select};
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, Instant};

mod report;

#[derive(Debug, Clone)]
pub struct Measurement {
    pub function_name: &'static str,
    pub duration: Duration,
}

#[derive(Debug, Clone)]
pub struct FunctionStats {
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub total_duration: Duration,
    pub count: u64,
}

impl FunctionStats {
    pub fn new(duration: Duration) -> Self {
        Self {
            min_duration: duration,
            max_duration: duration,
            total_duration: duration,
            count: 1,
        }
    }

    pub fn update(&mut self, duration: Duration) {
        self.min_duration = self.min_duration.min(duration);
        self.max_duration = self.max_duration.max(duration);
        self.total_duration += duration;
        self.count += 1;
    }

    pub fn avg_duration(&self) -> Duration {
        if self.count > 0 {
            let total_nanos = self.total_duration.as_nanos();
            let avg_nanos = total_nanos / self.count as u128;
            Duration::from_nanos(avg_nanos as u64)
        } else {
            Duration::ZERO
        }
    }
}

struct HotPathState {
    sender: Option<Sender<Measurement>>,
    shutdown_tx: Option<Sender<()>>,
    completion_rx: Option<Receiver<()>>,
    stats: Option<HashMap<String, FunctionStats>>, // Will be populated by worker at shutdown
    start_time: Instant,
    caller_name: String,
    shutdown_initiated: bool,
}

static HOTPATH_STATE: OnceLock<Arc<RwLock<HotPathState>>> = OnceLock::new();

pub struct HotPath {
    state: Arc<RwLock<HotPathState>>,
}

impl Drop for HotPath {
    fn drop(&mut self) {
        // Decrement ref count
        let state = Arc::clone(&self.state);

        // Signal shutdown and wait for processing thread to complete
        let (shutdown_tx, completion_rx) = {
            let Ok(mut state_guard) = state.write() else {
                // If state is poisoned, just return
                return;
            };

            // Make shutdown idempotent
            if state_guard.shutdown_initiated {
                return;
            }
            state_guard.shutdown_initiated = true;

            state_guard.sender = None;
            let shutdown_tx = state_guard.shutdown_tx.take();
            let completion_rx = state_guard.completion_rx.take();
            (shutdown_tx, completion_rx)
        };

        // Send shutdown signal (non-panicking)
        if let Some(tx) = shutdown_tx {
            let _ = tx.send(());
        }

        // Wait for processing thread to finish (non-panicking)
        if let Some(rx) = completion_rx {
            let _ = rx.recv();
        }

        // Display summary (non-panicking)
        if let Ok(state_guard) = state.read()
            && let Some(ref stats) = state_guard.stats
            && !stats.is_empty()
        {
            let total_elapsed = Instant::now().duration_since(state_guard.start_time);
            report::display_performance_summary(stats, total_elapsed, &state_guard.caller_name);
        }
    }
}

fn process_measurement(stats: &mut HashMap<String, FunctionStats>, m: Measurement) {
    if let Some(s) = stats.get_mut(m.function_name) {
        s.update(m.duration);
    } else {
        stats.insert(m.function_name.to_string(), FunctionStats::new(m.duration));
    }
}

#[macro_export]
macro_rules! init {
    () => {{
        fn __caller_fn() {}
        let caller_name = std::any::type_name_of_val(&__caller_fn);
        let caller_name = caller_name
            .strip_suffix("::__caller_fn")
            .unwrap_or(caller_name)
            .replace("::{{closure}}", "")
            .to_string();

        $crate::init_with_caller(caller_name)
    }};
}

pub fn init_with_caller(caller_name: String) -> HotPath {
    if HOTPATH_STATE.get().is_some() {
        panic!("hotpath::init() must be called only once");
    }

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
            shutdown_initiated: false,
        }));

        let state_clone = Arc::clone(&state_arc);
        thread::Builder::new()
            .name("hotpath-worker".into())
            .spawn(move || {
                let mut local_stats = HashMap::<String, FunctionStats>::new();

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

                // Signal completion
                let _ = completion_tx.send(());
            })
            .expect("Failed to spawn hotpath-worker thread");

        state_arc
    });

    HotPath {
        state: Arc::clone(state),
    }
}

pub fn send_measurement(name: &'static str, duration: Duration) {
    let Some(state) = HOTPATH_STATE.get() else {
        panic!("hotpath::init() must be called when --features hotpath is enabled");
    };

    let Ok(state_guard) = state.read() else {
        return;
    };
    let Some(sender) = state_guard.sender.as_ref() else {
        return;
    };

    let measurement = Measurement {
        function_name: name,
        duration,
    };
    let _ = sender.try_send(measurement);
}

#[macro_export]
macro_rules! measure_block {
    ($label:expr, $expr:expr) => {{
        // Enforce the label is a &'static str at compile-time
        let __label_static: &'static str = $label;

        let __t0 = ::std::time::Instant::now();
        let __hotpath_out = $expr;
        $crate::send_measurement(__label_static, __t0.elapsed());
        __hotpath_out
    }};
}
