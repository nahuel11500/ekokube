use anyhow::{Context, Result};

const DEFAULT_TOKEN_PATH: &str = "/var/run/secrets/kubernetes.io/serviceaccount/token";

#[derive(Clone, Debug)]
pub struct Config {
    pub node_name: String,
    pub interval_secs: u64,
    pub meta_interval_secs: u64,
    pub cgroup_root: String,
    pub kubelet_url: String,
    pub kubelet_token_path: String,
    pub kubelet_insecure_tls: bool,
    pub clickhouse_url: String,
    pub clickhouse_database: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub metrics_addr: String,
    pub buffer_max_rows: usize,
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let node_name = std::env::var("EKOKUBE_NODE_NAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .context("EKOKUBE_NODE_NAME (or HOSTNAME) must be set")?;
        Ok(Self {
            node_name,
            interval_secs: env_or("EKOKUBE_INTERVAL_SECS", "15")
                .parse()
                .context("EKOKUBE_INTERVAL_SECS must be an integer")?,
            meta_interval_secs: env_or("EKOKUBE_META_INTERVAL_SECS", "30")
                .parse()
                .context("EKOKUBE_META_INTERVAL_SECS must be an integer")?,
            cgroup_root: env_or("EKOKUBE_CGROUP_ROOT", "/sys/fs/cgroup"),
            kubelet_url: env_or("EKOKUBE_KUBELET_URL", "https://127.0.0.1:10250"),
            kubelet_token_path: env_or("EKOKUBE_KUBELET_TOKEN_PATH", DEFAULT_TOKEN_PATH),
            kubelet_insecure_tls: env_or("EKOKUBE_KUBELET_INSECURE_TLS", "true") == "true",
            clickhouse_url: env_or("EKOKUBE_CLICKHOUSE_URL", "http://localhost:8123"),
            clickhouse_database: env_or("EKOKUBE_CLICKHOUSE_DATABASE", "ekokube"),
            clickhouse_user: env_or("EKOKUBE_CLICKHOUSE_USER", "default"),
            clickhouse_password: env_or("EKOKUBE_CLICKHOUSE_PASSWORD", ""),
            metrics_addr: env_or("EKOKUBE_METRICS_ADDR", "0.0.0.0:9090"),
            buffer_max_rows: env_or("EKOKUBE_BUFFER_MAX_ROWS", "250000")
                .parse()
                .context("EKOKUBE_BUFFER_MAX_ROWS must be an integer")?,
        })
    }
}
