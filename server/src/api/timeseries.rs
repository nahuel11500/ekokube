//! Scoped time series (cluster / namespace / workload) for detail pages, and
//! batched sparkline series for table rows.

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::Json;
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::{choose_bucket, ApiError, AppState, RangeQuery};

#[derive(Row, Deserialize)]
struct Point {
    t: u32,
    cpu_usage: f64,
    cpu_request: f64,
    cpu_limit: f64,
    mem_ws: f64,
    mem_request: f64,
    mem_limit: f64,
}

#[derive(Serialize, Default)]
pub struct TimeseriesResponse {
    step: u32,
    ts: Vec<u32>,
    cpu_usage_millicores: Vec<f64>,
    cpu_request_millicores: Vec<f64>,
    cpu_limit_millicores: Vec<f64>,
    mem_working_set_bytes: Vec<f64>,
    mem_request_bytes: Vec<f64>,
    mem_limit_bytes: Vec<f64>,
}

fn scope_filter(query: &RangeQuery) -> (String, Vec<String>) {
    let mut sql = String::new();
    let mut binds = Vec::new();
    if let Some(ns) = &query.namespace {
        sql.push_str(" AND namespace = ?");
        binds.push(ns.clone());
    }
    if let Some(wl) = &query.workload {
        sql.push_str(" AND workload_name = ?");
        binds.push(wl.clone());
    }
    (sql, binds)
}

pub async fn handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<TimeseriesResponse>, ApiError> {
    let (from, to) = query.range();
    let (step, raw) = choose_bucket(from, to);
    let daily = to.saturating_sub(from) > 10 * 24 * 3600;
    let step = if daily { 24 * 3600 } else { step };
    let from = from / step * step;
    let to = (to / step * step).max(from + step);
    let (scope, binds) = scope_filter(&query);

    let sql = if daily {
        format!(
            "SELECT toDateTime(day) AS t, \
                    sum(cpu_core_secs)*1000/{step} AS cpu_usage, \
                    sum(cpu_request_core_secs)*1000/{step} AS cpu_request, \
                    sum(cpu_limit_core_secs)*1000/{step} AS cpu_limit, \
                    sum(mem_ws_byte_secs)/{step} AS mem_ws, \
                    sum(mem_request_byte_secs)/{step} AS mem_request, \
                    sum(mem_limit_byte_secs)/{step} AS mem_limit \
             FROM pod_usage_1d \
             WHERE day >= toDate(toDateTime(?)) AND day < toDate(toDateTime(?)){scope} \
             GROUP BY t ORDER BY t"
        )
    } else if raw {
        format!(
            "SELECT toStartOfInterval(ts, INTERVAL {step} SECOND) AS t, \
                    sum(cpu_usage_millicores * sample_secs)/{step} AS cpu_usage, \
                    sum(cpu_request_millicores * sample_secs)/{step} AS cpu_request, \
                    sum(cpu_limit_millicores * sample_secs)/{step} AS cpu_limit, \
                    sum(mem_working_set_bytes * sample_secs)/{step} AS mem_ws, \
                    sum(mem_request_bytes * sample_secs)/{step} AS mem_request, \
                    sum(mem_limit_bytes * sample_secs)/{step} AS mem_limit \
             FROM pod_usage WHERE ts >= toDateTime(?) AND ts < toDateTime(?){scope} \
             GROUP BY t ORDER BY t"
        )
    } else {
        format!(
            "SELECT toStartOfInterval(hour, INTERVAL {step} SECOND) AS t, \
                    sum(cpu_core_secs)*1000/{step} AS cpu_usage, \
                    sum(cpu_request_core_secs)*1000/{step} AS cpu_request, \
                    sum(cpu_limit_core_secs)*1000/{step} AS cpu_limit, \
                    sum(mem_ws_byte_secs)/{step} AS mem_ws, \
                    sum(mem_request_byte_secs)/{step} AS mem_request, \
                    sum(mem_limit_byte_secs)/{step} AS mem_limit \
             FROM pod_usage_1h WHERE hour >= toDateTime(?) AND hour < toDateTime(?){scope} \
             GROUP BY t ORDER BY t"
        )
    };
    let mut request = state.ch.query(&sql).bind(from).bind(to);
    for b in &binds {
        request = request.bind(b.as_str());
    }
    let points: Vec<Point> = request.fetch_all().await?;

    let mut response = TimeseriesResponse {
        step,
        ..Default::default()
    };
    for p in points {
        response.ts.push(p.t);
        response.cpu_usage_millicores.push(p.cpu_usage);
        response.cpu_request_millicores.push(p.cpu_request);
        response.cpu_limit_millicores.push(p.cpu_limit);
        response.mem_working_set_bytes.push(p.mem_ws);
        response.mem_request_bytes.push(p.mem_request);
        response.mem_limit_bytes.push(p.mem_limit);
    }
    Ok(Json(response))
}

// ---- sparklines: one small usage series per table row, in one query ----

#[derive(Deserialize)]
pub struct SparklineQuery {
    pub from: Option<u32>,
    pub to: Option<u32>,
    /// "namespace" (default) or "workload" (requires namespace)
    pub entity: Option<String>,
    pub namespace: Option<String>,
}

#[derive(Row, Deserialize)]
struct SparkPoint {
    key: String,
    t: u32,
    cpu: f64,
}

#[derive(Serialize)]
pub struct SparklineSeries {
    key: String,
    ts: Vec<u32>,
    cpu_millicores: Vec<f64>,
}

pub async fn sparklines(
    State(state): State<AppState>,
    Query(query): Query<SparklineQuery>,
) -> Result<Json<Vec<SparklineSeries>>, ApiError> {
    let range = RangeQuery {
        from: query.from,
        to: query.to,
        namespace: None,
        workload: None,
        sort_by: None,
        order: None,
        limit: None,
        offset: None,
    };
    let (from, to) = range.range();
    // ~24 points per sparkline is plenty at row height.
    let step = (to.saturating_sub(from) / 24).max(60);
    let from = from / step * step;
    let to = (to / step * step).max(from + step);

    let by_workload = query.entity.as_deref() == Some("workload");
    let key_col = if by_workload {
        "workload_name"
    } else {
        "namespace"
    };
    let (table, tcol, factor) = if to.saturating_sub(from) <= 48 * 3600 {
        ("pod_usage", "ts", "cpu_usage_millicores * sample_secs")
    } else {
        ("pod_usage_1h", "hour", "cpu_core_secs * 1000")
    };
    let ns_filter = if by_workload {
        " AND namespace = ?"
    } else {
        ""
    };
    let sql = format!(
        "SELECT {key_col} AS key, toStartOfInterval({tcol}, INTERVAL {step} SECOND) AS t, \
                sum({factor})/{step} AS cpu \
         FROM {table} WHERE {tcol} >= toDateTime(?) AND {tcol} < toDateTime(?){ns_filter} \
         GROUP BY key, t ORDER BY key, t"
    );
    let mut request = state.ch.query(&sql).bind(from).bind(to);
    if by_workload {
        let ns = query.namespace.as_deref().unwrap_or("");
        request = request.bind(ns);
    }
    let points: Vec<SparkPoint> = request.fetch_all().await?;

    let mut series: HashMap<String, SparklineSeries> = HashMap::new();
    let mut order: Vec<String> = Vec::new();
    for p in points {
        let entry = series.entry(p.key.clone()).or_insert_with(|| {
            order.push(p.key.clone());
            SparklineSeries {
                key: p.key.clone(),
                ts: Vec::new(),
                cpu_millicores: Vec::new(),
            }
        });
        entry.ts.push(p.t);
        entry.cpu_millicores.push(p.cpu);
    }
    Ok(Json(
        order
            .into_iter()
            .filter_map(|k| series.remove(&k))
            .collect(),
    ))
}
