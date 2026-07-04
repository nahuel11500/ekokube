use anyhow::{Context, Result};

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[derive(Clone, Debug)]
pub struct Config {
    pub listen_addr: String,
    pub clickhouse_url: String,
    pub clickhouse_database: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub sync_enabled: bool,
    pub sync_interval_secs: u64,
    /// HTTP Basic Auth for the UI + API. Enabled when a username is set.
    pub auth_username: String,
    pub auth_password: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            listen_addr: env_or("EKOKUBE_LISTEN_ADDR", "0.0.0.0:8080"),
            clickhouse_url: env_or("EKOKUBE_CLICKHOUSE_URL", "http://localhost:8123"),
            clickhouse_database: env_or("EKOKUBE_CLICKHOUSE_DATABASE", "ekokube"),
            clickhouse_user: env_or("EKOKUBE_CLICKHOUSE_USER", "default"),
            clickhouse_password: env_or("EKOKUBE_CLICKHOUSE_PASSWORD", ""),
            sync_enabled: env_or("EKOKUBE_SYNC_ENABLED", "true") == "true",
            sync_interval_secs: env_or("EKOKUBE_SYNC_INTERVAL_SECS", "60")
                .parse()
                .context("EKOKUBE_SYNC_INTERVAL_SECS must be an integer")?,
            auth_username: env_or("EKOKUBE_AUTH_USERNAME", ""),
            auth_password: env_or("EKOKUBE_AUTH_PASSWORD", ""),
        })
    }
}
