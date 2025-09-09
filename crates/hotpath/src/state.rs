use crossbeam_channel::{Receiver, Sender, bounded, select};
use hdrhistogram::Histogram;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

#[cfg(feature = "hotpath-alloc")]
use crate::alloc::core::AllocationInfo;

#[derive(Debug)]
pub enum Measurement {
    Duration(u64, &'static str), // duration_ns, function_name
    #[cfg(feature = "hotpath-alloc")]
    Allocation(&'static str, AllocationInfo), // function_name, allocation_info
}

#[derive(Debug, Clone)]
pub struct FunctionStats {
    pub count: u64,
    // Time tracking fields
    pub total_duration_ns: u64,
    duration_hist: Option<Histogram<u64>>,
    pub has_data: bool,
    // Allocation tracking fields
    #[cfg(feature = "hotpath-alloc")]
    bytes_total_hist: Option<Histogram<u64>>,
    #[cfg(feature = "hotpath-alloc")]
    bytes_max_hist: Option<Histogram<u64>>,
    #[cfg(feature = "hotpath-alloc")]
    pub has_alloc_data: bool,
}

impl FunctionStats {
    const LOW_NS: u64 = 1;
    const HIGH_NS: u64 = 10_000_000_000; // 10s
    #[cfg(feature = "hotpath-alloc")]
    const LOW_BYTES: u64 = 1;
    #[cfg(feature = "hotpath-alloc")]
    const HIGH_BYTES: u64 = 1_000_000_000; // 1GB
    const SIGFIGS: u8 = 3;

    pub fn new_duration(first_ns: u64) -> Self {
        let hist = Histogram::<u64>::new_with_bounds(Self::LOW_NS, Self::HIGH_NS, Self::SIGFIGS)
            .expect("hdrhistogram init");

        let mut s = Self {
            total_duration_ns: first_ns,
            count: 1,
            duration_hist: Some(hist),
            has_data: true,
            #[cfg(feature = "hotpath-alloc")]
            bytes_total_hist: None,
            #[cfg(feature = "hotpath-alloc")]
            bytes_max_hist: None,
            #[cfg(feature = "hotpath-alloc")]
            has_alloc_data: false,
        };
        s.record_time(first_ns);
        s
    }

    #[cfg(feature = "hotpath-alloc")]
    pub fn new_alloc(alloc_info: &AllocationInfo) -> Self {
        let bytes_total_hist =
            Histogram::<u64>::new_with_bounds(Self::LOW_BYTES, Self::HIGH_BYTES, Self::SIGFIGS)
                .expect("bytes_total histogram init");
        let bytes_max_hist =
            Histogram::<u64>::new_with_bounds(Self::LOW_BYTES, Self::HIGH_BYTES, Self::SIGFIGS)
                .expect("bytes_max histogram init");

        let mut s = Self {
            count: 1,
            total_duration_ns: 0,
            duration_hist: None,
            has_data: false,
            bytes_total_hist: Some(bytes_total_hist),
            bytes_max_hist: Some(bytes_max_hist),
            has_alloc_data: true,
        };
        s.record_alloc(alloc_info);
        s
    }

    #[inline]
    fn record_time(&mut self, ns: u64) {
        if let Some(ref mut hist) = self.duration_hist {
            let clamped = ns.clamp(Self::LOW_NS, Self::HIGH_NS);
            hist.record(clamped).unwrap();
        }
    }

    #[cfg(feature = "hotpath-alloc")]
    #[inline]
    fn record_alloc(&mut self, alloc_info: &AllocationInfo) {
        if let Some(ref mut bytes_total_hist) = self.bytes_total_hist
            && alloc_info.bytes_total > 0
        {
            let clamped_total = alloc_info
                .bytes_total
                .clamp(Self::LOW_BYTES, Self::HIGH_BYTES);
            bytes_total_hist.record(clamped_total).unwrap();
        }
        if let Some(ref mut bytes_max_hist) = self.bytes_max_hist
            && alloc_info.bytes_max > 0
        {
            let clamped_max = alloc_info
                .bytes_max
                .clamp(Self::LOW_BYTES, Self::HIGH_BYTES);
            bytes_max_hist.record(clamped_max).unwrap();
        }
    }

    pub fn update_duration(&mut self, duration_ns: u64) {
        self.total_duration_ns += duration_ns;
        self.count += 1;
        self.record_time(duration_ns);
    }

    #[cfg(feature = "hotpath-alloc")]
    pub fn update_alloc(&mut self, alloc_info: &AllocationInfo) {
        self.count += 1;
        self.record_alloc(alloc_info);
    }

    pub fn avg_duration_ns(&self) -> u64 {
        if self.count == 0 {
            0
        } else {
            self.total_duration_ns / self.count
        }
    }

    #[inline]
    pub fn percentile(&self, p: f64) -> Duration {
        if self.count == 0 || self.duration_hist.is_none() {
            return Duration::ZERO;
        }
        let p = p.clamp(0.0, 100.0);
        let v = self.duration_hist.as_ref().unwrap().value_at_percentile(p);
        Duration::from_nanos(v)
    }

    #[cfg(feature = "hotpath-alloc")]
    #[inline]
    pub fn bytes_total_percentile(&self, p: f64) -> u64 {
        if self.count == 0 || self.bytes_total_hist.is_none() {
            return 0;
        }
        let p = p.clamp(0.0, 100.0);
        self.bytes_total_hist
            .as_ref()
            .unwrap()
            .value_at_percentile(p)
    }

    #[cfg(feature = "hotpath-alloc")]
    #[inline]
    pub fn bytes_max_percentile(&self, p: f64) -> u64 {
        if self.count == 0 || self.bytes_max_hist.is_none() {
            return 0;
        }
        let p = p.clamp(0.0, 100.0);
        self.bytes_max_hist.as_ref().unwrap().value_at_percentile(p)
    }
}

pub struct HotPathState {
    pub sender: Option<Sender<Measurement>>,
    pub shutdown_tx: Option<Sender<()>>,
    pub completion_rx: Option<Receiver<()>>,
    pub stats: Option<HashMap<&'static str, FunctionStats>>,
    pub start_time: Instant,
    pub caller_name: String,
    pub percentiles: Vec<u8>,
}

fn process_measurement(stats: &mut HashMap<&'static str, FunctionStats>, m: Measurement) {
    match m {
        Measurement::Duration(duration_ns, name) => {
            if let Some(s) = stats.get_mut(name) {
                s.update_duration(duration_ns);
            } else {
                stats.insert(name, FunctionStats::new_duration(duration_ns));
            }
        }
        #[cfg(feature = "hotpath-alloc")]
        Measurement::Allocation(name, alloc_info) => {
            if let Some(s) = stats.get_mut(name) {
                s.update_alloc(&alloc_info);
            } else {
                stats.insert(name, FunctionStats::new_alloc(&alloc_info));
            }
        }
    }
}

use crate::HOTPATH_STATE;
use crate::HotPath;

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

pub fn send_duration_measurement(name: &'static str, duration: Duration) {
    let Some(state) = HOTPATH_STATE.get() else {
        panic!("hotpath::init() must be called when --features hotpath is enabled");
    };

    let Ok(state_guard) = state.read() else {
        return;
    };
    let Some(sender) = state_guard.sender.as_ref() else {
        return;
    };

    let measurement = Measurement::Duration(duration.as_nanos() as u64, name);
    let _ = sender.try_send(measurement);
}

#[cfg(feature = "hotpath-alloc")]
pub fn send_alloc_measurement(name: &'static str, alloc_info: AllocationInfo) {
    let Some(state) = HOTPATH_STATE.get() else {
        panic!("hotpath::init() must be called when --features hotpath is enabled");
    };

    let Ok(state_guard) = state.read() else {
        return;
    };
    let Some(sender) = state_guard.sender.as_ref() else {
        return;
    };

    let measurement = Measurement::Allocation(name, alloc_info);
    let _ = sender.try_send(measurement);
}
