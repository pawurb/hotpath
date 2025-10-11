use std::time::Instant;

#[doc(hidden)]
pub struct MeasurementGuard {
    name: &'static str,
    start: Instant,
    wrapper: bool,
}

impl MeasurementGuard {
    #[inline]
    pub fn new(name: &'static str, wrapper: bool, _unsupported_sync: bool) -> Self {
        Self {
            name,
            start: Instant::now(),
            wrapper,
        }
    }
}

impl Drop for MeasurementGuard {
    #[inline]
    fn drop(&mut self) {
        let dur = self.start.elapsed();
        super::state::send_duration_measurement(self.name, dur, self.wrapper);
    }
}
