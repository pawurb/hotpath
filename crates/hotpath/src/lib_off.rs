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

pub struct MeasurementGuard {}

impl MeasurementGuard {
    pub fn new(_name: &'static str, _wrapper: bool, _unsupported_async: bool) -> Self {
        Self {}
    }

    pub fn build(_name: &'static str, _wrapper: bool) -> Self {
        Self {}
    }
}

pub struct HotPath;

impl HotPath {
    pub fn new() -> Self {
        Self {}
    }
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

#[derive(Debug, Clone)]
pub struct FunctionStats {}
