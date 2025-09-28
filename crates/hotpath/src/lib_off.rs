pub use cfg_if::cfg_if;
pub use hotpath_macros::{main, measure};

#[macro_export]
macro_rules! measure_block {
    ($label:expr, $expr:expr) => {{
        $expr
    }};
}

#[derive(Clone, Copy, Debug, Default)]
pub enum Format {
    #[default]
    Table,
    Json,
    JsonPretty,
}

pub fn init(_caller_name: String, _percentiles: &[u8], _format: Format) -> HotPath {
    HotPath
}

pub struct HotPath;

impl Drop for HotPath {
    fn drop(&mut self) {}
}

pub struct GuardBuilder {}

impl GuardBuilder {
    pub fn new(_caller_name: impl Into<String>) -> Self {
        Self {}
    }

    pub fn percentiles(self, _percentiles: &[u8]) -> Self {
        self
    }

    pub fn format(self, _format: Format) -> Self {
        self
    }

    pub fn build(self) -> HotPath {
        HotPath
    }
}
