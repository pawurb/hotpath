pub use hotpath_macros::{main, measure};

use crossbeam_channel::{Receiver, Sender, bounded, select};
use hdrhistogram::Histogram;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use std::thread;
use std::time::{Duration, Instant};

mod report;

pub type Measurement = (u64, &'static str);

#[derive(Debug)]
pub struct FunctionStats {
    pub total_duration_ns: u64,
    pub count: u64,
    hist: Histogram<u64>,
}

impl FunctionStats {
    const LOW_NS: u64 = 1;
    const HIGH_NS: u64 = 10_000_000_000; // 10s
    const SIGFIGS: u8 = 3;

    pub fn new(first_ns: u64) -> Self {
        let hist = Histogram::<u64>::new_with_bounds(Self::LOW_NS, Self::HIGH_NS, Self::SIGFIGS)
            .expect("hdrhistogram init");
        let mut s = Self {
            total_duration_ns: first_ns,
            count: 1,
            hist,
        };
        s.record(first_ns);
        s
    }

    #[inline]
    fn record(&mut self, ns: u64) {
        let clamped = ns.clamp(Self::LOW_NS, Self::HIGH_NS);
        self.hist.record(clamped).unwrap();
    }

    pub fn update(&mut self, duration_ns: u64) {
        self.total_duration_ns += duration_ns;
        self.count += 1;
        self.record(duration_ns);
    }

    pub fn avg_duration_ns(&self) -> u64 {
        if self.count == 0 {
            0
        } else {
            self.total_duration_ns / self.count
        }
    }

    /// Percentile in [1.0, 99.0], e.g. 95.0 or 99.0
    #[inline]
    pub fn percentile(&self, p: f64) -> Duration {
        if self.count == 0 {
            return Duration::ZERO;
        }
        let p = p.clamp(0.0, 100.0);
        let v = self.hist.value_at_percentile(p);
        Duration::from_nanos(v)
    }
}

pub struct MeasureGuard {
    name: &'static str,
    start: Instant,
}

impl MeasureGuard {
    #[inline]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
        }
    }
}

impl Drop for MeasureGuard {
    #[inline]
    fn drop(&mut self) {
        let dur = self.start.elapsed();
        crate::send_measurement(self.name, dur);
    }
}

struct HotPathState {
    sender: Option<Sender<Measurement>>,
    shutdown_tx: Option<Sender<()>>,
    completion_rx: Option<Receiver<()>>,
    stats: Option<HashMap<&'static str, FunctionStats>>, // Will be populated by worker at shutdown
    start_time: Instant,
    caller_name: String,
    percentiles: Vec<u8>,
}

static HOTPATH_STATE: OnceLock<Arc<RwLock<HotPathState>>> = OnceLock::new();

pub struct HotPath {
    state: Arc<RwLock<HotPathState>>,
}

impl Drop for HotPath {
    fn drop(&mut self) {
        let state = Arc::clone(&self.state);

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
            let _ = rx.recv();
        }

        if let Ok(state_guard) = state.read()
            && let Some(ref stats) = state_guard.stats
        {
            let total_elapsed = end_time.duration_since(state_guard.start_time);
            if stats.is_empty() {
                report::display_no_measurements_message(total_elapsed, &state_guard.caller_name);
            } else {
                report::display_performance_summary(
                    stats,
                    total_elapsed,
                    &state_guard.caller_name,
                    &state_guard.percentiles,
                );
            }
        }
    }
}

fn process_measurement(stats: &mut HashMap<&'static str, FunctionStats>, m: Measurement) {
    let duration_ns = m.0;
    if let Some(s) = stats.get_mut(m.1) {
        s.update(duration_ns);
    } else {
        stats.insert(m.1, FunctionStats::new(duration_ns));
    }
}

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

    let measurement = (duration.as_nanos() as u64, name);
    let _ = sender.try_send(measurement);
}

#[macro_export]
macro_rules! measure_block {
    ($label:expr, $expr:expr) => {{
        let __label_static: &'static str = $label;
        let __t0 = ::std::time::Instant::now();
        let __hotpath_out = $expr;
        $crate::send_measurement(__label_static, __t0.elapsed());
        __hotpath_out
    }};
}
