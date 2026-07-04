//! Per-pod stats for the workload detail page. Averages are over each pod's
//! own observed seconds (a pod's truthful rate while it ran), not the query
//! window — right-sizing needs "what does this pod do when alive".

use axum::extract::{Query, State};
use axum::Json;
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState, RangeQuery};

const SORT_COLUMNS: &[&str] = &[
    "cpu_usage_avg",
    "cpu_p95",
    "mem_usage_avg",
    "mem_ws_max",
    "cpu_throttled_max",
    "runtime_secs",
    "pod_name",
];

#[derive(Row, Deserialize)]
struct PodStat {
    pod_name: String,
    node: String,
    qos: String,
    runtime_secs: f64,
    cpu_usage_avg: f64,
    cpu_p95: f32,
    cpu_request: f64,
    cpu_limit: f64,
    cpu_throttled_max: f32,
    mem_usage_avg: f64,
    mem_ws_max: u64,
    mem_request: f64,
    psi_cpu_max: f32,
    psi_mem_max: f32,
}

#[derive(Serialize, Default)]
pub struct PodsResponse {
    pod_name: Vec<String>,
    node: Vec<String>,
    qos: Vec<String>,
    runtime_secs: Vec<f64>,
    cpu_usage_avg_millicores: Vec<f64>,
    cpu_p95_millicores: Vec<f32>,
    cpu_request_millicores: Vec<f64>,
    cpu_limit_millicores: Vec<f64>,
    cpu_throttled_max_ratio: Vec<f32>,
    mem_usage_avg_bytes: Vec<f64>,
    mem_max_bytes: Vec<u64>,
    mem_request_bytes: Vec<f64>,
    psi_cpu_max_ratio: Vec<f32>,
    psi_mem_max_ratio: Vec<f32>,
    has_more: bool,
}

pub async fn handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<PodsResponse>, ApiError> {
    let (from, to) = query.pod_range();
    let (table, _) = query.pod_source();
    let where_clause = query.pod_where();
    let sort = query.sort_sql(SORT_COLUMNS, "cpu_usage_avg");
    let limit = query.limit();
    let mut filters = String::new();
    if query.namespace.is_some() {
        filters.push_str(" AND namespace = ?");
    }
    if query.workload.is_some() {
        filters.push_str(" AND workload_name = ?");
    }

    // Requests/limits divide by each pod's secs (they're constant per pod, so
    // this recovers the configured value exactly).
    let sql = format!(
        "SELECT pod_name, any(node) AS node, any(qos) AS qos, \
                sum(secs) AS runtime_secs, \
                sum(cpu_core_secs)*1000/sum(secs) AS cpu_usage_avg, \
                quantileTDigestMerge(0.95)(cpu_usage_p95) AS cpu_p95, \
                sum(cpu_request_core_secs)*1000/sum(secs) AS cpu_request, \
                sum(cpu_limit_core_secs)*1000/sum(secs) AS cpu_limit, \
                max(cpu_throttled_max) AS cpu_throttled_max, \
                sum(mem_ws_byte_secs)/sum(secs) AS mem_usage_avg, \
                max(mem_ws_max) AS mem_ws_max, \
                sum(mem_request_byte_secs)/sum(secs) AS mem_request, \
                max(psi_cpu_some_max) AS psi_cpu_max, \
                max(psi_mem_some_max) AS psi_mem_max \
         FROM {table} \
         WHERE {where_clause}{filters} \
         GROUP BY pod_name \
         ORDER BY {sort} \
         LIMIT ? OFFSET ?"
    );
    let mut request = state.ch.query(&sql).bind(from).bind(to);
    if let Some(ns) = &query.namespace {
        request = request.bind(ns.as_str());
    }
    if let Some(wl) = &query.workload {
        request = request.bind(wl.as_str());
    }
    let mut rows: Vec<PodStat> = request
        .bind(limit + 1)
        .bind(query.offset())
        .fetch_all()
        .await?;
    let has_more = rows.len() as u32 > limit;
    rows.truncate(limit as usize);

    let mut response = PodsResponse {
        has_more,
        ..Default::default()
    };
    for r in rows {
        response.pod_name.push(r.pod_name);
        response.node.push(r.node);
        response.qos.push(r.qos);
        response.runtime_secs.push(r.runtime_secs);
        response.cpu_usage_avg_millicores.push(r.cpu_usage_avg);
        response.cpu_p95_millicores.push(r.cpu_p95);
        response.cpu_request_millicores.push(r.cpu_request);
        response.cpu_limit_millicores.push(r.cpu_limit);
        response.cpu_throttled_max_ratio.push(r.cpu_throttled_max);
        response.mem_usage_avg_bytes.push(r.mem_usage_avg);
        response.mem_max_bytes.push(r.mem_ws_max);
        response.mem_request_bytes.push(r.mem_request);
        response.psi_cpu_max_ratio.push(r.psi_cpu_max);
        response.psi_mem_max_ratio.push(r.psi_mem_max);
    }
    Ok(Json(response))
}
