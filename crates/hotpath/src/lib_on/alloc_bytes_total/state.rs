use crossbeam_channel::{Receiver, Sender};
use hdrhistogram::Histogram;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

pub enum Measurement {
    Allocation(&'static str, u64, bool, bool), // function_name, bytes_total, unsupported_async, wrapper
}

#[derive(Debug, Clone)]
pub struct FunctionStats {
    pub count: u64,
    bytes_total_hist: Option<Histogram<u64>>,
    pub has_data: bool,
    pub has_unsupported_async: bool,
    pub wrapper: bool,
}

impl FunctionStats {
    const LOW_BYTES: u64 = 1;
    const HIGH_BYTES: u64 = 1_000_000_000; // 1GB
    const SIGFIGS: u8 = 3;

    pub fn new_alloc(bytes_total: u64, unsupported_async: bool, wrapper: bool) -> Self {
        let bytes_total_hist =
            Histogram::<u64>::new_with_bounds(Self::LOW_BYTES, Self::HIGH_BYTES, Self::SIGFIGS)
                .expect("bytes_total histogram init");

        let mut s = Self {
            count: 1,
            bytes_total_hist: Some(bytes_total_hist),
            has_data: true,
            has_unsupported_async: unsupported_async,
            wrapper,
        };
        s.record_alloc(bytes_total);
        s
    }

    #[inline]
    fn record_alloc(&mut self, bytes_total: u64) {
        if let Some(ref mut bytes_total_hist) = self.bytes_total_hist {
            if bytes_total > 0 {
                let clamped_total = bytes_total.clamp(Self::LOW_BYTES, Self::HIGH_BYTES);
                bytes_total_hist.record(clamped_total).unwrap();
            }
        }
    }

    pub fn update_alloc(&mut self, bytes_total: u64, unsupported_async: bool) {
        self.count += 1;
        self.has_unsupported_async |= unsupported_async;
        self.record_alloc(bytes_total);
    }

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

    #[inline]
    pub fn total_bytes(&self) -> u64 {
        if self.count == 0 || self.bytes_total_hist.is_none() {
            return 0;
        }
        // For total bytes allocation, we sum up the mean * count to get total
        let hist = self.bytes_total_hist.as_ref().unwrap();
        let mean = hist.mean();
        (mean * self.count as f64) as u64
    }

    #[inline]
    pub fn avg_bytes(&self) -> u64 {
        if self.count == 0 || self.bytes_total_hist.is_none() {
            return 0;
        }
        self.bytes_total_hist.as_ref().unwrap().mean() as u64
    }
}

pub(crate) struct HotPathState {
    pub sender: Option<Sender<Measurement>>,
    pub shutdown_tx: Option<Sender<()>>,
    pub completion_rx: Option<Mutex<Receiver<HashMap<&'static str, FunctionStats>>>>,
    pub start_time: Instant,
    pub caller_name: &'static str,
    pub percentiles: Vec<u8>,
    pub limit: usize,
}

pub(crate) fn process_measurement(
    stats: &mut HashMap<&'static str, FunctionStats>,
    m: Measurement,
) {
    match m {
        Measurement::Allocation(name, bytes_total, unsupported_async, wrapper) => {
            if let Some(s) = stats.get_mut(name) {
                s.update_alloc(bytes_total, unsupported_async);
            } else {
                stats.insert(
                    name,
                    FunctionStats::new_alloc(bytes_total, unsupported_async, wrapper),
                );
            }
        }
    }
}

use crate::lib_on::HOTPATH_STATE;

pub fn send_alloc_measurement(
    name: &'static str,
    bytes_total: u64,
    unsupported_async: bool,
    wrapper: bool,
) {
    let Some(arc_swap) = HOTPATH_STATE.get() else {
        panic!(
            "GuardBuilder::new(\"main\").build() must be called when --features hotpath-alloc-bytes-total is enabled"
        );
    };

    let Some(state) = arc_swap.load_full() else {
        return;
    };

    let Ok(state_guard) = state.read() else {
        return;
    };
    let Some(sender) = state_guard.sender.as_ref() else {
        return;
    };

    let measurement = Measurement::Allocation(name, bytes_total, unsupported_async, wrapper);
    let _ = sender.try_send(measurement);
}
