use crate::output::MetricsJson;
use crate::HOTPATH_STATE;
use crossbeam_channel::bounded;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Display;
use std::thread;
use std::time::Duration;
use tiny_http::{Header, Request, Response, Server};

pub fn start_metrics_server(port: u16) {
    thread::Builder::new()
        .name("hotpath-http-server".into())
        .spawn(move || {
            let addr = format!("0.0.0.0:{}", port);
            let server = match Server::http(&addr) {
                Ok(s) => s,
                Err(e) => {
                    panic!(
                        "Failed to bind metrics server to {}: {}. Customize the port using the HOTPATH_HTTP_PORT environment variable.",
                        addr, e
                    );
                }
            };

            eprintln!("[hotpath] Metrics server listening on http://{}", addr);

            for request in server.incoming_requests() {
                handle_request(request);
            }
        })
        .expect("Failed to spawn HTTP metrics server thread");
}

fn handle_request(request: Request) {
    let path = request.url().split('?').next().unwrap_or("/");

    match path {
        "/metrics" => {
            let metrics = get_current_metrics();
            respond_json(request, &metrics);
        }
        _ => {
            respond_error(request, 404, "Not found");
        }
    }
}

fn respond_json<T: Serialize>(request: Request, value: &T) {
    match serde_json::to_vec(value) {
        Ok(body) => {
            let mut response = Response::from_data(body);
            response.add_header(
                Header::from_bytes(b"Content-Type".as_slice(), b"application/json".as_slice())
                    .unwrap(),
            );
            let _ = request.respond(response);
        }
        Err(e) => respond_internal_error(request, e),
    }
}

fn respond_error(request: Request, code: u16, msg: &str) {
    let _ = request.respond(Response::from_string(msg).with_status_code(code));
}

fn respond_internal_error(request: Request, e: impl Display) {
    eprintln!("Internal server error: {}", e);
    let _ = request.respond(
        Response::from_string(format!("Internal server error: {}", e)).with_status_code(500),
    );
}

fn get_current_metrics() -> MetricsJson {
    if let Some(metrics) = try_get_metrics_from_worker() {
        return metrics;
    }

    // Fallback if query fails: return empty metrics
    MetricsJson {
        hotpath_profiling_mode: crate::output::ProfilingMode::Timing,
        total_elapsed: 0,
        description: "No metrics available yet".to_string(),
        caller_name: "hotpath".to_string(),
        percentiles: vec![95],
        data: crate::output::MetricsDataJson(HashMap::new()),
    }
}

fn try_get_metrics_from_worker() -> Option<MetricsJson> {
    let arc_swap = HOTPATH_STATE.get()?;
    let state_option = arc_swap.load();
    let state_arc = (*state_option).as_ref()?.clone();

    let state_guard = state_arc.read().ok()?;

    let (response_tx, response_rx) = bounded::<MetricsJson>(1);

    if let Some(query_tx) = &state_guard.query_tx {
        query_tx.send(response_tx).ok()?;
        drop(state_guard);

        response_rx.recv_timeout(Duration::from_millis(100)).ok()
    } else {
        None
    }
}
