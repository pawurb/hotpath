use hotpath::MetricsJson;
use ratatui::widgets::TableState;
use std::time::Instant;

pub struct App {
    pub metrics: MetricsJson,
    pub table_state: TableState,
    pub paused: bool,
    pub last_refresh: Instant,
    pub last_successful_fetch: Option<Instant>,
    pub error_message: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            metrics: MetricsJson {
                hotpath_profiling_mode: hotpath::ProfilingMode::Timing,
                total_elapsed: 0,
                description: "Waiting for data...".to_string(),
                caller_name: "unknown".to_string(),
                percentiles: vec![95],
                data: hotpath::MetricsDataJson(std::collections::HashMap::new()),
            },
            table_state: TableState::default(),
            paused: false,
            last_refresh: Instant::now(),
            last_successful_fetch: None,
            error_message: None,
        }
    }

    pub fn next_function(&mut self) {
        let function_count = self.metrics.data.0.len();
        if function_count == 0 {
            return;
        }

        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= function_count - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn previous_function(&mut self) {
        let function_count = self.metrics.data.0.len();
        if function_count == 0 {
            return;
        }

        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    function_count - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn update_metrics(&mut self, metrics: MetricsJson) {
        self.metrics = metrics;
        self.last_successful_fetch = Some(Instant::now());
        self.error_message = None;
    }

    pub fn set_error(&mut self, error: String) {
        self.error_message = Some(error);
    }
}
