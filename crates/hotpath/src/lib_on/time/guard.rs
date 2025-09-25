use std::time::Instant;

pub struct TimeGuard {
    name: &'static str,
    start: Instant,
}

impl TimeGuard {
    #[inline]
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            start: Instant::now(),
        }
    }
}

impl Drop for TimeGuard {
    #[inline]
    fn drop(&mut self) {
        let dur = self.start.elapsed();
        super::state::send_duration_measurement(self.name, dur);
    }
}
