mod cgroup;
mod collector;
mod config;
mod kubelet;
mod metrics;
mod model;
mod node;
mod sink;

use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let config = config::Config::from_env()?;
    tracing::info!(node = %config.node_name, interval = config.interval_secs, "starting ekokube-agent");

    let shared_metrics = Arc::new(metrics::Metrics::default());
    tokio::spawn(metrics::serve(
        config.metrics_addr.clone(),
        shared_metrics.clone(),
    ));

    let collector = collector::Collector::new(config, shared_metrics)?;
    collector.run().await
}
