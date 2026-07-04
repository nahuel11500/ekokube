mod api;
mod config;
mod syncer;
mod tenancy;

use anyhow::{Context, Result};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    // The kube client builds a rustls TLS config from the process-level default
    // CryptoProvider; without this it panics ("could not determine provider").
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install rustls crypto provider");

    let config = config::Config::from_env()?;
    let ch = clickhouse::Client::default()
        .with_url(&config.clickhouse_url)
        .with_database(&config.clickhouse_database)
        .with_user(&config.clickhouse_user)
        .with_password(&config.clickhouse_password);

    if config.sync_enabled {
        let ch = ch.clone();
        let interval = config.sync_interval_secs;
        tokio::spawn(async move {
            if let Err(e) = syncer::run(ch, interval).await {
                tracing::error!(error = %e, "namespace/node syncer stopped");
            }
        });
    } else {
        tracing::info!("kubernetes syncer disabled (EKOKUBE_SYNC_ENABLED != true)");
    }

    let auth = api::BasicAuth::from_config(&config.auth_username, &config.auth_password);
    if auth.is_some() {
        tracing::info!(user = %config.auth_username, "basic auth enabled");
    } else {
        tracing::warn!("basic auth DISABLED (set EKOKUBE_AUTH_USERNAME/PASSWORD to enable)");
    }
    let app = api::router(ch, auth);
    let listener = tokio::net::TcpListener::bind(&config.listen_addr)
        .await
        .with_context(|| format!("binding {}", config.listen_addr))?;
    tracing::info!(addr = %config.listen_addr, "ekokube-server listening");
    axum::serve(listener, app).await?;
    Ok(())
}
