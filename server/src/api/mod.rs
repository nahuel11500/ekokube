mod auth;
mod metrics;
mod namespaces;
mod nodes;
mod overview;
mod rules;
mod tenants;
mod workloads;

pub use auth::BasicAuth;

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use tower_http::compression::CompressionLayer;

#[derive(Clone)]
pub struct AppState {
    pub ch: clickhouse::Client,
    pub metrics: Arc<metrics::RequestMetrics>,
}

/// Readiness: the server is only useful if ClickHouse answers.
async fn ready(State(state): State<AppState>) -> Result<&'static str, ApiError> {
    state.ch.query("SELECT 1").fetch_one::<u8>().await?;
    Ok("ready")
}

pub fn router(ch: clickhouse::Client, auth: Option<BasicAuth>) -> Router {
    let state = AppState {
        ch,
        metrics: Arc::new(metrics::RequestMetrics::default()),
    };
    Router::new()
        .route("/api/health", get(|| async { "ok" }))
        .route("/api/health/ready", get(ready))
        .route("/metrics", get(metrics::handler))
        .route("/api/overview", get(overview::handler))
        .route("/api/namespaces", get(namespaces::handler))
        .route("/api/workloads", get(workloads::handler))
        .route("/api/nodes", get(nodes::handler))
        .route("/api/tenants", get(tenants::handler))
        .route("/api/tenants/export.csv", get(tenants::export_csv))
        .route("/api/rules", get(rules::get_rules).put(rules::put_rules))
        .route("/api/rules/preview", post(rules::preview_rules))
        // The production image ships the built UI next to the binary; in dev
        // the Vite server proxies /api instead and this serves nothing.
        .fallback_service(tower_http::services::ServeDir::new(
            std::env::var("EKOKUBE_STATIC_DIR").unwrap_or_else(|_| "./static".into()),
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            metrics::track,
        ))
        .layer(CompressionLayer::new())
        // Auth is the outermost layer (added last → runs first), so unauthorized
        // requests are rejected before reaching handlers. UI is served
        // same-origin, so no CORS layer is needed.
        .layer(axum::middleware::from_fn_with_state(auth, auth::guard))
        .with_state(state)
}

/// Errors bubble up as 500 with the message in the body.
pub struct ApiError(anyhow::Error);

impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        Self(e.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        tracing::error!(error = %self.0, "api error");
        (StatusCode::INTERNAL_SERVER_ERROR, format!("{:#}", self.0)).into_response()
    }
}

fn now_epoch() -> u32 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

/// Common query parameters. Times are unix seconds; default range is the last 24h.
#[derive(Deserialize, Debug)]
pub struct RangeQuery {
    pub from: Option<u32>,
    pub to: Option<u32>,
    pub namespace: Option<String>,
    pub sort_by: Option<String>,
    pub order: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl RangeQuery {
    pub fn range(&self) -> (u32, u32) {
        let to = self.to.unwrap_or_else(now_epoch);
        let from = self.from.unwrap_or(to.saturating_sub(24 * 3600));
        (from.min(to), to)
    }

    /// Rollup granularity for pod-usage table queries. Ranges beyond a few
    /// days read the daily rollup (`pod_usage_1d`) so a 30-day query scans
    /// ~200k×30 rows instead of ~200k×720 — the difference between <500ms and
    /// several seconds at scale. Both rollups share the same aggregate columns
    /// (resource-seconds), so only the table and time column change.
    fn use_daily(&self) -> bool {
        let (from, to) = self.range();
        to.saturating_sub(from) > 3 * 24 * 3600
    }

    /// (table, time column) for pod-usage rollup queries.
    pub fn pod_source(&self) -> (&'static str, &'static str) {
        if self.use_daily() {
            ("pod_usage_1d", "day")
        } else {
            ("pod_usage_1h", "hour")
        }
    }

    /// Range snapped down to the rollup bucket boundary (hour or day). A rollup
    /// row can only be counted whole, so an unaligned `from` would drop the
    /// boundary bucket. window_secs() uses the same bounds, so the numerator
    /// (Σ resource-seconds) and denominator stay consistent — averages are
    /// exact for the aligned span.
    pub fn pod_range(&self) -> (u32, u32) {
        let (from, to) = self.range();
        let bucket = if self.use_daily() { 24 * 3600 } else { 3600 };
        let from = from / bucket * bucket;
        (from, to.max(from + 60))
    }

    /// WHERE fragment (two `?` bound to pod_range from/to) matching pod_source's
    /// time column type: DateTime for hourly, Date for daily. Keeps the raw
    /// column in the predicate so partition pruning still applies.
    pub fn pod_where(&self) -> String {
        let (_, col) = self.pod_source();
        if self.use_daily() {
            format!("{col} >= toDate(toDateTime(?)) AND {col} < toDate(toDateTime(?))")
        } else {
            format!("{col} >= toDateTime(?) AND {col} < toDateTime(?)")
        }
    }

    /// Window length in seconds; Σ(resource_secs) / window = avg concurrent usage.
    pub fn window_secs(&self) -> f64 {
        let (from, to) = self.pod_range();
        (to - from) as f64
    }

    /// Rollup bucket width in seconds for the selected pod source.
    pub fn bucket_secs(&self) -> u32 {
        if self.use_daily() {
            24 * 3600
        } else {
            3600
        }
    }

    pub fn limit(&self) -> u32 {
        self.limit.unwrap_or(50).min(500)
    }

    pub fn offset(&self) -> u32 {
        self.offset.unwrap_or(0)
    }

    /// Whitelists the sort column; never interpolates user input into SQL.
    pub fn sort_sql(&self, allowed: &[&'static str], default: &'static str) -> String {
        let column = self
            .sort_by
            .as_deref()
            .and_then(|s| allowed.iter().find(|c| **c == s))
            .copied()
            .unwrap_or(default);
        let order = match self.order.as_deref() {
            Some("asc") => "ASC",
            _ => "DESC",
        };
        format!("{column} {order}")
    }
}

#[derive(clickhouse::Row, serde::Deserialize)]
struct Coverage {
    mn: u32,
    mx: u32,
    cnt: u64,
}

/// Denominator for "average concurrent" columns: the part of the requested
/// window actually covered by data. On a fresh install, dividing by the full
/// window dilutes requests/usage/waste to near zero (a 1-core request shows
/// as ~20mc after 30 minutes of collection in a 24h view) — misleading, since
/// requests are a setting, not consumption. Once data spans the whole window
/// this equals window_secs(), so long-running installs are unaffected.
/// Chargeback core-hours are absolute sums and never touch this.
pub async fn effective_window_secs(
    ch: &clickhouse::Client,
    query: &RangeQuery,
) -> Result<f64, ApiError> {
    let (from, to) = query.pod_range();

    // Exact probe first: raw node_usage records precisely when agents were
    // collecting (one narrow row per node per cycle — cheap to scan even at
    // fleet scale, unlike raw pod_usage). Counting distinct collection
    // minutes (not the min→max span) keeps gaps — agent downtime, ClickHouse
    // outages past the buffer — out of the denominator.
    let raw: Coverage = ch
        .query(
            "SELECT toUInt32(uniqExact(toStartOfMinute(ts)) * 60) AS mn, toUInt32(0) AS mx, count() AS cnt \
             FROM node_usage WHERE ts >= toDateTime(?) AND ts < toDateTime(?)",
        )
        .bind(from)
        .bind(to)
        .fetch_one()
        .await?;
    if raw.cnt > 0 {
        return Ok((raw.mn as f64).max(60.0));
    }

    // Range predates the raw TTL: approximate from rollup buckets (±1 bucket).
    let (table, col) = query.pod_source();
    let bucket = query.bucket_secs();
    let time_expr = if col == "day" {
        format!("toDateTime({col})")
    } else {
        col.to_string()
    };
    let sql = format!(
        "SELECT toUInt32(min({time_expr})) AS mn, toUInt32(max({time_expr})) AS mx, count() AS cnt \
         FROM {table} WHERE {}",
        query.pod_where()
    );
    let coverage: Coverage = ch.query(&sql).bind(from).bind(to).fetch_one().await?;
    if coverage.cnt == 0 {
        return Ok(query.window_secs());
    }
    let start = coverage.mn.max(from);
    let end = (coverage.mx + bucket).min(to.max(start + 60));
    Ok((end.saturating_sub(start) as f64).max(60.0))
}

/// Picks a bucket size (seconds) targeting ~360 points, and whether the raw
/// tables can serve it (raw data is only retained 14 days).
pub fn choose_bucket(from: u32, to: u32) -> (u32, bool) {
    let range = to.saturating_sub(from).max(60);
    let raw = range <= 48 * 3600;
    let step = if raw {
        (range / 360).max(15) / 15 * 15
    } else {
        (range / 360).max(3600) / 3600 * 3600
    };
    (step, raw)
}
