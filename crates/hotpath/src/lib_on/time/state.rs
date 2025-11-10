use crossbeam_channel::{Receiver, Sender};
use hdrhistogram::Histogram;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub enum Measurement {
    Duration(u64, &'static str, bool), // duration_ns, function_name, wrapper
}

#[derive(Debug)]
pub struct FunctionStats {
    pub total_duration_ns: u64,
    pub count: u64,
    hist: Option<Histogram<u64>>,
    pub has_data: bool,
    pub wrapper: bool,
    pub recent_samples: VecDeque<u64>,
}

impl FunctionStats {
    const LOW_NS: u64 = 1;
    const HIGH_NS: u64 = 1_000_000_000_000; // 1000s
    const SIGFIGS: u8 = 3;

    pub fn new_duration(first_ns: u64, wrapper: bool, recent_samples_limit: usize) -> Self {
        let hist = Histogram::<u64>::new_with_bounds(Self::LOW_NS, Self::HIGH_NS, Self::SIGFIGS)
            .expect("hdrhistogram init");

        let mut recent_samples = VecDeque::with_capacity(recent_samples_limit);
        recent_samples.push_back(first_ns);

        let mut s = Self {
            total_duration_ns: first_ns,
            count: 1,
            hist: Some(hist),
            has_data: true,
            wrapper,
            recent_samples,
        };
        s.record_time(first_ns);
        s
    }

    #[inline]
    fn record_time(&mut self, ns: u64) {
        if let Some(ref mut hist) = self.hist {
            let clamped = ns.clamp(Self::LOW_NS, Self::HIGH_NS);
            hist.record(clamped).unwrap();
        }
    }

    pub fn update_duration(&mut self, duration_ns: u64) {
        self.total_duration_ns += duration_ns;
        self.count += 1;
        self.record_time(duration_ns);

        if self.recent_samples.len() == self.recent_samples.capacity()
            && self.recent_samples.capacity() > 0
        {
            self.recent_samples.pop_front();
        }
        self.recent_samples.push_back(duration_ns);
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
        if self.count == 0 || self.hist.is_none() {
            return Duration::ZERO;
        }
        let p = p.clamp(0.0, 100.0);
        let v = self.hist.as_ref().unwrap().value_at_percentile(p);
        Duration::from_nanos(v)
    }
}

pub(crate) struct HotPathState {
    pub sender: Option<Sender<Measurement>>,
    pub shutdown_tx: Option<Sender<()>>,
    pub completion_rx: Option<Mutex<Receiver<HashMap<&'static str, FunctionStats>>>>,
    pub query_tx: Option<Sender<super::super::QueryRequest>>,
    pub start_time: Instant,
    pub caller_name: &'static str,
    pub percentiles: Vec<u8>,
    pub limit: usize,
    pub recent_samples_limit: usize,
}

pub(crate) fn process_measurement(
    stats: &mut HashMap<&'static str, FunctionStats>,
    m: Measurement,
    recent_samples_limit: usize,
) {
    match m {
        Measurement::Duration(duration_ns, name, wrapper) => {
            if let Some(s) = stats.get_mut(name) {
                s.update_duration(duration_ns);
            } else {
                stats.insert(
                    name,
                    FunctionStats::new_duration(duration_ns, wrapper, recent_samples_limit),
                );
            }
        }
    }
}

use super::super::HOTPATH_STATE;

pub fn send_duration_measurement(name: &'static str, duration: Duration, wrapper: bool) {
    let Some(arc_swap) = HOTPATH_STATE.get() else {
        panic!(
            "GuardBuilder::new(\"main\").build() must be called when --features hotpath is enabled"
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

    let measurement = Measurement::Duration(duration.as_nanos() as u64, name, wrapper);
    let _ = sender.try_send(measurement);
}
