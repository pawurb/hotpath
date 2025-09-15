use crossbeam_channel::{Receiver, Sender};
use hdrhistogram::Histogram;
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub enum Measurement {
    Duration(u64, &'static str), // duration_ns, function_name
}

#[derive(Debug)]
pub struct FunctionStats {
    pub total_duration_ns: u64,
    pub count: u64,
    hist: Option<Histogram<u64>>,
    pub has_data: bool,
}

impl FunctionStats {
    const LOW_NS: u64 = 1;
    const HIGH_NS: u64 = 10_000_000_000; // 10s
    const SIGFIGS: u8 = 3;

    pub fn new_duration(first_ns: u64) -> Self {
        let hist = Histogram::<u64>::new_with_bounds(Self::LOW_NS, Self::HIGH_NS, Self::SIGFIGS)
            .expect("hdrhistogram init");

        let mut s = Self {
            total_duration_ns: first_ns,
            count: 1,
            hist: Some(hist),
            has_data: true,
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

pub struct HotPathState {
    pub sender: Option<Sender<Measurement>>,
    pub shutdown_tx: Option<Sender<()>>,
    pub completion_rx: Option<Receiver<HashMap<&'static str, FunctionStats>>>,
    pub start_time: Instant,
    pub caller_name: String,
    pub percentiles: Vec<u8>,
    pub format: crate::Format,
}

pub(crate) fn process_measurement(
    stats: &mut HashMap<&'static str, FunctionStats>,
    m: Measurement,
) {
    match m {
        Measurement::Duration(duration_ns, name) => {
            if let Some(s) = stats.get_mut(name) {
                s.update_duration(duration_ns);
            } else {
                stats.insert(name, FunctionStats::new_duration(duration_ns));
            }
        }
    }
}

use crate::HOTPATH_STATE;

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
