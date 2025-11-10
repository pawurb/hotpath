use hotpath::{MetricsJson, SamplesJson};
use ratatui::widgets::TableState;
use std::time::Instant;

pub(crate) struct App {
    pub(crate) metrics: MetricsJson,
    pub(crate) table_state: TableState,
    pub(crate) paused: bool,
    pub(crate) last_refresh: Instant,
    pub(crate) last_successful_fetch: Option<Instant>,
    pub(crate) error_message: Option<String>,
    pub(crate) show_samples: bool,
    pub(crate) current_samples: Option<SamplesJson>,
    pub(crate) pinned_function: Option<String>,
}

impl App {
    pub(crate) fn new() -> Self {
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
            show_samples: false,
            current_samples: None,
            pinned_function: None,
        }
    }

    pub(crate) fn next_function(&mut self) {
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

    pub(crate) fn previous_function(&mut self) {
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

    pub(crate) fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub(crate) fn update_metrics(&mut self, metrics: MetricsJson) {
        self.metrics = metrics;
        self.last_successful_fetch = Some(Instant::now());
        self.error_message = None;
    }

    pub(crate) fn set_error(&mut self, error: String) {
        self.error_message = Some(error);
    }

    pub(crate) fn toggle_samples(&mut self) {
        self.show_samples = !self.show_samples;
        if self.show_samples {
            // Pin the currently selected function when opening samples panel
            self.pinned_function = self.selected_function_name();
        } else {
            // Clear pinned function when closing samples panel
            self.pinned_function = None;
        }
    }

    pub(crate) fn selected_function_name(&self) -> Option<String> {
        self.table_state
            .selected()
            .and_then(|idx| self.metrics.data.0.keys().nth(idx).map(|s| s.to_string()))
    }

    pub(crate) fn update_samples(&mut self, samples: SamplesJson) {
        self.current_samples = Some(samples);
    }

    pub(crate) fn clear_samples(&mut self) {
        self.current_samples = None;
    }

    pub(crate) fn update_pinned_function(&mut self) {
        if self.show_samples {
            self.pinned_function = self.selected_function_name();
        }
    }

    pub(crate) fn samples_function_name(&self) -> Option<&str> {
        self.pinned_function.as_deref()
    }

    /// Fetch samples for pinned function if panel is open
    pub(crate) fn fetch_samples_if_open(&mut self, port: u16) {
        if self.show_samples {
            if let Some(function_name) = self.samples_function_name() {
                match super::http::fetch_samples(port, function_name) {
                    Ok(samples) => self.update_samples(samples),
                    Err(_) => self.clear_samples(),
                }
            }
        }
    }

    /// Update pinned function and fetch samples if panel is open
    pub(crate) fn update_and_fetch_samples(&mut self, port: u16) {
        self.update_pinned_function();
        self.fetch_samples_if_open(port);
    }
}
