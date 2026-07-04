//! Self-observability: hand-rolled Prometheus text exposition over a minimal
//! HTTP listener (no web framework in the agent's dependency tree).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Default)]
pub struct Metrics {
    pub cycles_total: AtomicU64,
    /// Duration of the last collection cycle, milliseconds.
    pub cycle_duration_ms: AtomicU64,
    /// Pods scraped in the last cycle.
    pub pods_scraped: AtomicU64,
    pub pod_usage_rows_total: AtomicU64,
    pub node_usage_rows_total: AtomicU64,
    pub pod_meta_rows_total: AtomicU64,
    pub insert_failures_total: AtomicU64,
    /// Rows currently waiting in the outage buffer.
    pub buffer_rows: AtomicU64,
    pub buffer_dropped_rows_total: AtomicU64,
}

impl Metrics {
    pub fn rows_counter(&self, table: &str) -> &AtomicU64 {
        match table {
            "pod_usage" => &self.pod_usage_rows_total,
            "node_usage" => &self.node_usage_rows_total,
            _ => &self.pod_meta_rows_total,
        }
    }

    pub fn render(&self) -> String {
        let g = |a: &AtomicU64| a.load(Ordering::Relaxed);
        format!(
            "# TYPE ekokube_agent_cycles_total counter\n\
             ekokube_agent_cycles_total {}\n\
             # TYPE ekokube_agent_cycle_duration_seconds gauge\n\
             ekokube_agent_cycle_duration_seconds {}\n\
             # TYPE ekokube_agent_pods_scraped gauge\n\
             ekokube_agent_pods_scraped {}\n\
             # TYPE ekokube_agent_rows_written_total counter\n\
             ekokube_agent_rows_written_total{{table=\"pod_usage\"}} {}\n\
             ekokube_agent_rows_written_total{{table=\"node_usage\"}} {}\n\
             ekokube_agent_rows_written_total{{table=\"pod_meta\"}} {}\n\
             # TYPE ekokube_agent_insert_failures_total counter\n\
             ekokube_agent_insert_failures_total {}\n\
             # TYPE ekokube_agent_buffer_rows gauge\n\
             ekokube_agent_buffer_rows {}\n\
             # TYPE ekokube_agent_buffer_dropped_rows_total counter\n\
             ekokube_agent_buffer_dropped_rows_total {}\n",
            g(&self.cycles_total),
            g(&self.cycle_duration_ms) as f64 / 1000.0,
            g(&self.pods_scraped),
            g(&self.pod_usage_rows_total),
            g(&self.node_usage_rows_total),
            g(&self.pod_meta_rows_total),
            g(&self.insert_failures_total),
            g(&self.buffer_rows),
            g(&self.buffer_dropped_rows_total),
        )
    }
}

/// Serves GET /metrics. Minimal by design: reads the request head, always
/// answers with the current exposition.
pub async fn serve(addr: String, metrics: Arc<Metrics>) {
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(addr, error = %e, "metrics listener failed to bind");
            return;
        }
    };
    tracing::info!(addr, "metrics endpoint listening");
    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            continue;
        };
        let metrics = metrics.clone();
        tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            let body = metrics.render();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/plain; version=0.0.4\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });
    }
}
