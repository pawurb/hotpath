use crate::alloc_count_total::core::AllocationInfo;
use crossbeam_channel::{Receiver, Sender};
use hdrhistogram::Histogram;
use std::collections::HashMap;
use std::time::Instant;

pub enum Measurement {
    Allocation(&'static str, AllocationInfo), // function_name, allocation_info
}

#[derive(Debug, Clone)]
pub struct FunctionStats {
    pub count: u64,
    count_total_hist: Option<Histogram<u64>>,
    pub has_data: bool,
}

impl FunctionStats {
    const LOW_COUNT: u64 = 1;
    const HIGH_COUNT: u64 = 1_000_000_000; // 1 billion allocations
    const SIGFIGS: u8 = 3;

    pub fn new_alloc(alloc_info: &AllocationInfo) -> Self {
        let count_total_hist =
            Histogram::<u64>::new_with_bounds(Self::LOW_COUNT, Self::HIGH_COUNT, Self::SIGFIGS)
                .expect("count_total histogram init");

        let mut s = Self {
            count: 1,
            count_total_hist: Some(count_total_hist),
            has_data: true,
        };
        s.record_alloc(alloc_info);
        s
    }

    #[inline]
    fn record_alloc(&mut self, alloc_info: &AllocationInfo) {
        if let Some(ref mut count_total_hist) = self.count_total_hist
            && alloc_info.count_total > 0
        {
            let clamped_total = alloc_info
                .count_total
                .clamp(Self::LOW_COUNT, Self::HIGH_COUNT);
            count_total_hist.record(clamped_total).unwrap();
        }
    }

    pub fn update_alloc(&mut self, alloc_info: &AllocationInfo) {
        self.count += 1;
        self.record_alloc(alloc_info);
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

pub struct HotPathState {
    pub sender: Option<Sender<Measurement>>,
    pub shutdown_tx: Option<Sender<()>>,
    pub completion_rx: Option<Receiver<()>>,
    pub stats: Option<HashMap<&'static str, FunctionStats>>,
    pub start_time: Instant,
    pub caller_name: String,
    pub percentiles: Vec<u8>,
}

pub(crate) fn process_measurement(
    stats: &mut HashMap<&'static str, FunctionStats>,
    m: Measurement,
) {
    match m {
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

pub fn send_alloc_measurement(name: &'static str, alloc_info: AllocationInfo) {
    let Some(state) = HOTPATH_STATE.get() else {
        panic!(
            "hotpath::init() must be called when --features hotpath-alloc-count-total is enabled"
        );
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
