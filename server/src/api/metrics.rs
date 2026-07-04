//! Request metrics (count + total duration per route) and Prometheus exposition.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::Response;

use super::AppState;

#[derive(Default)]
pub struct RequestMetrics {
    /// route -> (request count, total duration in microseconds)
    routes: Mutex<HashMap<String, (u64, u64)>>,
}

impl RequestMetrics {
    fn record(&self, route: &str, micros: u64) {
        let mut routes = self.routes.lock().unwrap();
        let entry = routes.entry(route.to_string()).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += micros;
    }

    pub fn render(&self) -> String {
        let routes = self.routes.lock().unwrap();
        let mut out = String::from(
            "# TYPE ekokube_http_requests_total counter\n\
             # TYPE ekokube_http_request_duration_seconds_sum counter\n",
        );
        let mut sorted: Vec<_> = routes.iter().collect();
        sorted.sort_by_key(|(route, _)| route.as_str());
        for (route, (count, micros)) in sorted {
            out.push_str(&format!(
                "ekokube_http_requests_total{{route=\"{route}\"}} {count}\n\
                 ekokube_http_request_duration_seconds_sum{{route=\"{route}\"}} {}\n",
                *micros as f64 / 1e6
            ));
        }
        out
    }
}

pub async fn track(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let route = request.uri().path().to_string();
    let started = Instant::now();
    let response = next.run(request).await;
    if route.starts_with("/api/") {
        state
            .metrics
            .record(&route, started.elapsed().as_micros() as u64);
    }
    response
}

pub async fn handler(State(state): State<AppState>) -> String {
    state.metrics.render()
}
