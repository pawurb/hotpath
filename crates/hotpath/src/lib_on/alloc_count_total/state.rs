use crossbeam_channel::{Receiver, Sender};
use hdrhistogram::Histogram;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

pub enum Measurement {
    Allocation(&'static str, u64, bool, bool, bool), // function_name, count_total, unsupported_async, wrapper, cross_thread
}

#[derive(Debug, Clone)]
pub struct FunctionStats {
    pub count: u64,
    count_total_hist: Option<Histogram<u64>>,
    pub has_data: bool,
    pub has_unsupported_async: bool,
    pub wrapper: bool,
    pub cross_thread: bool,
}

impl FunctionStats {
    const LOW_COUNT: u64 = 1;
    const HIGH_COUNT: u64 = 1_000_000_000; // 1 billion allocations
    const SIGFIGS: u8 = 3;

    pub fn new_alloc(
        count_total: u64,
        unsupported_async: bool,
        wrapper: bool,
        cross_thread: bool,
    ) -> Self {
        let count_total_hist =
            Histogram::<u64>::new_with_bounds(Self::LOW_COUNT, Self::HIGH_COUNT, Self::SIGFIGS)
                .expect("count_total histogram init");

        let mut s = Self {
            count: 1,
            count_total_hist: Some(count_total_hist),
            has_data: true,
            has_unsupported_async: unsupported_async,
            wrapper,
            cross_thread,
        };
        s.record_alloc(count_total);
        s
    }

    #[inline]
    fn record_alloc(&mut self, count_total: u64) {
        if let Some(ref mut count_total_hist) = self.count_total_hist {
            if count_total > 0 {
                let clamped_total = count_total.clamp(Self::LOW_COUNT, Self::HIGH_COUNT);
                count_total_hist.record(clamped_total).unwrap();
            }
        }
    }

    pub fn update_alloc(&mut self, count_total: u64, unsupported_async: bool, cross_thread: bool) {
        self.count += 1;
        self.has_unsupported_async |= unsupported_async;
        self.cross_thread |= cross_thread;
        self.record_alloc(count_total);
    }

    #[inline]
    pub fn count_total_percentile(&self, p: f64) -> u64 {
        if self.count == 0 || self.count_total_hist.is_none() {
            return 0;
        }
        let p = p.clamp(0.0, 100.0);
        self.count_total_hist
            .as_ref()
            .unwrap()
            .value_at_percentile(p)
    }

    #[inline]
    pub fn total_count(&self) -> u64 {
        if self.count == 0 || self.count_total_hist.is_none() {
            return 0;
        }
        // For total count allocation, we sum up the mean * count to get total
        let hist = self.count_total_hist.as_ref().unwrap();
        let mean = hist.mean();
        (mean * self.count as f64) as u64
    }

    #[inline]
    pub fn avg_count(&self) -> u64 {
        if self.count == 0 || self.count_total_hist.is_none() {
            return 0;
        }
        self.count_total_hist.as_ref().unwrap().mean() as u64
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
        Measurement::Allocation(name, count_total, unsupported_async, wrapper, cross_thread) => {
            if let Some(s) = stats.get_mut(name) {
                s.update_alloc(count_total, unsupported_async, cross_thread);
            } else {
                stats.insert(
                    name,
                    FunctionStats::new_alloc(count_total, unsupported_async, wrapper, cross_thread),
                );
            }
        }
    }
}

use crate::lib_on::HOTPATH_STATE;

pub fn send_alloc_measurement(
    name: &'static str,
    count_total: u64,
    unsupported_async: bool,
    wrapper: bool,
    cross_thread: bool,
) {
    let Some(arc_swap) = HOTPATH_STATE.get() else {
        panic!(
            "GuardBuilder::new(\"main\").build() must be called when --features hotpath-alloc-count-total is enabled"
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

    let measurement =
        Measurement::Allocation(name, count_total, unsupported_async, wrapper, cross_thread);
    let _ = sender.try_send(measurement);
}
