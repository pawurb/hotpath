use std::time::Instant;

#[doc(hidden)]
pub struct TimeGuard {
    name: &'static str,
    start: Instant,
    wrapper: bool,
}

impl TimeGuard {
    #[inline]
    pub fn new(name: &'static str, wrapper: bool) -> Self {
        Self {
            name,
            start: Instant::now(),
            wrapper,
        }
    }
}

impl Drop for TimeGuard {
    #[inline]
    fn drop(&mut self) {
        let dur = self.start.elapsed();
        super::state::send_duration_measurement(self.name, dur, self.wrapper);
    }
}
