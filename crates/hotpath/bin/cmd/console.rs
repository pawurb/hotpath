mod app;
mod http;
mod views;
mod widgets;

use app::App;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use eyre::Result;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::{Duration, Instant};

#[derive(Debug, Parser)]
pub struct ConsoleArgs {
    #[arg(
        long,
        default_value_t = 6770,
        help = "Port where the metrics HTTP server is running"
    )]
    pub metrics_port: u16,

    #[arg(long, default_value_t = 500, help = "Refresh interval in milliseconds")]
    pub refresh_interval: u64,
}

impl ConsoleArgs {
    pub fn run(&self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut app = App::new();

        let result = run_tui(
            &mut terminal,
            &mut app,
            self.metrics_port,
            self.refresh_interval,
        );

        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }
}

fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    port: u16,
    refresh_interval_ms: u64,
) -> Result<()> {
    let refresh_interval = Duration::from_millis(refresh_interval_ms);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| views::render_ui(f, app))?;

        let timeout = refresh_interval
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => {
                        return Ok(());
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        app.next_function();
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        app.previous_function();
                    }
                    KeyCode::Char('p') | KeyCode::Char('P') => {
                        app.toggle_pause();
                    }
                    _ => {}
                }
            }
        }

        if last_tick.elapsed() >= refresh_interval {
            if !app.paused {
                match http::fetch_metrics(port) {
                    Ok(metrics) => {
                        app.update_metrics(metrics);
                    }
                    Err(e) => {
                        app.set_error(format!("{}", e));
                    }
                }
            }

            app.last_refresh = Instant::now();
            last_tick = Instant::now();
        }
    }
}
