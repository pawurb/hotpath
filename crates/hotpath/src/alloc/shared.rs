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
    fn drop(&mut self) {
        cfg_if::cfg_if! {
            if #[cfg(feature = "hotpath-alloc-bytes-max")] {
                let alloc_info = crate::alloc_bytes_max::core::AllocationInfo {
                    bytes_current: 0,
                    bytes_max: 0,
                    unsupported_async: true,
                };
                crate::alloc_bytes_max::state::send_alloc_measurement(self.name, alloc_info);
            } else if #[cfg(feature = "hotpath-alloc-bytes-total")] {
                let alloc_info = crate::alloc_bytes_total::core::AllocationInfo {
                    bytes_total: 0,
                    unsupported_async: true,
                };
                crate::alloc_bytes_total::state::send_alloc_measurement(self.name, alloc_info);
            } else if #[cfg(feature = "hotpath-alloc-count-max")] {
                let alloc_info = crate::alloc_count_max::core::AllocationInfo {
                    count_current: 0,
                    count_max: 0,
                    unsupported_async: true,
                };
                crate::alloc_count_max::state::send_alloc_measurement(self.name, alloc_info);
            } else if #[cfg(feature = "hotpath-alloc-count-total")] {
                let alloc_info = crate::alloc_count_total::core::AllocationInfo {
                    count_total: 0,
                    unsupported_async: true,
                };
                crate::alloc_count_total::state::send_alloc_measurement(self.name, alloc_info);
            }
        }
    }
}
