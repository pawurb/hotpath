use super::super::app::App;
use super::{bottom_bar, functions, samples, top_bar};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame,
};

pub(crate) fn render_ui(frame: &mut Frame, app: &mut App) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Status bar
            Constraint::Min(0),    // Main content area
            Constraint::Length(3), // Help bar
        ])
        .split(frame.area());

    top_bar::render_status_bar(
        frame,
        main_chunks[0],
        app.paused,
        &app.error_message,
        &app.last_successful_fetch,
        app.last_refresh,
    );

    if app.show_samples {
        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(main_chunks[1]);

        functions::render_functions_table(frame, app, content_chunks[0], app.focus);
        samples::render_samples_panel(frame, content_chunks[1], app, app.focus);
    } else {
        functions::render_functions_table(frame, app, main_chunks[1], app.focus);
    }

    bottom_bar::render_help_bar(frame, main_chunks[2], app.focus, app.show_samples);
}
