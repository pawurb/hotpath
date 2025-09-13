pub fn format_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }

    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[unit_idx])
    } else {
        format!("{:.1} {}", size, UNITS[unit_idx])
    }
}

pub struct NoopAsyncAllocGuard {
    name: &'static str,
}

impl NoopAsyncAllocGuard {
    #[inline]
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl Drop for NoopAsyncAllocGuard {
    #[inline]
    fn drop(&mut self) {}
}
