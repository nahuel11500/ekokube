//! Per-workload consumption, waste, throttling and pressure (hourly rollups).

use axum::extract::{Query, State};
use axum::Json;
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState, RangeQuery};

const SORT_COLUMNS: &[&str] = &[
    "cpu_usage_avg",
    "cpu_request_avg",
    "cpu_waste",
    "cpu_p95",
    "cpu_throttled_max",
    "mem_usage_avg",
    "mem_request_avg",
    "mem_waste",
    "psi_cpu_max",
    "psi_mem_max",
    "pods",
];

#[derive(Row, Deserialize)]
struct WorkloadStat {
    namespace: String,
    workload_kind: String,
    workload_name: String,
    pods: u64,
    cpu_usage_avg: f64,
    cpu_request_avg: f64,
    cpu_waste: f64,
    cpu_p95: f32,
    cpu_throttled_max: f32,
    mem_usage_avg: f64,
    mem_request_avg: f64,
    mem_waste: f64,
    mem_ws_max: u64,
    psi_cpu_max: f32,
    psi_mem_max: f32,
}

#[derive(Serialize, Default)]
pub struct WorkloadsResponse {
    namespace: Vec<String>,
    workload_kind: Vec<String>,
    workload_name: Vec<String>,
    pods: Vec<u64>,
    cpu_usage_avg_millicores: Vec<f64>,
    cpu_request_avg_millicores: Vec<f64>,
    cpu_waste_millicores: Vec<f64>,
    cpu_p95_per_pod_millicores: Vec<f32>,
    cpu_throttled_max_ratio: Vec<f32>,
    mem_usage_avg_bytes: Vec<f64>,
    mem_request_avg_bytes: Vec<f64>,
    mem_waste_bytes: Vec<f64>,
    mem_max_per_pod_bytes: Vec<u64>,
    psi_cpu_max_ratio: Vec<f32>,
    psi_mem_max_ratio: Vec<f32>,
    has_more: bool,
}

pub async fn handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<WorkloadsResponse>, ApiError> {
    let (from, to) = query.pod_range();
    let window = query.window_secs();
    let sort = query.sort_sql(SORT_COLUMNS, "cpu_waste");
    let limit = query.limit();
    let (table, _) = query.pod_source();
    let where_clause = query.pod_where();
    let namespace_filter = match &query.namespace {
        Some(_) => "AND namespace = ?",
        None => "",
    };

    let sql = format!(
        "SELECT namespace, workload_kind, workload_name, \
                uniqExact(pod_uid) AS pods, \
                sum(cpu_core_secs)*1000/{window} AS cpu_usage_avg, \
                sum(cpu_request_core_secs)*1000/{window} AS cpu_request_avg, \
                cpu_request_avg - cpu_usage_avg AS cpu_waste, \
                quantileTDigestMerge(0.95)(cpu_usage_p95) AS cpu_p95, \
                max(cpu_throttled_max) AS cpu_throttled_max, \
                sum(mem_ws_byte_secs)/{window} AS mem_usage_avg, \
                sum(mem_request_byte_secs)/{window} AS mem_request_avg, \
                mem_request_avg - mem_usage_avg AS mem_waste, \
                max(mem_ws_max) AS mem_ws_max, \
                max(psi_cpu_some_max) AS psi_cpu_max, \
                max(psi_mem_some_max) AS psi_mem_max \
         FROM {table} \
         WHERE {where_clause} {namespace_filter} \
         GROUP BY namespace, workload_kind, workload_name \
         ORDER BY {sort} \
         LIMIT ? OFFSET ?"
    );
    let mut request = state.ch.query(&sql).bind(from).bind(to);
    if let Some(ns) = &query.namespace {
        request = request.bind(ns.as_str());
    }
    let mut rows: Vec<WorkloadStat> = request
        .bind(limit + 1)
        .bind(query.offset())
        .fetch_all()
        .await?;

    let has_more = rows.len() as u32 > limit;
    rows.truncate(limit as usize);

    let mut response = WorkloadsResponse {
        has_more,
        ..Default::default()
    };
    for r in rows {
        response.namespace.push(r.namespace);
        response.workload_kind.push(r.workload_kind);
        response.workload_name.push(r.workload_name);
        response.pods.push(r.pods);
        response.cpu_usage_avg_millicores.push(r.cpu_usage_avg);
        response.cpu_request_avg_millicores.push(r.cpu_request_avg);
        response.cpu_waste_millicores.push(r.cpu_waste);
        response.cpu_p95_per_pod_millicores.push(r.cpu_p95);
        response.cpu_throttled_max_ratio.push(r.cpu_throttled_max);
        response.mem_usage_avg_bytes.push(r.mem_usage_avg);
        response.mem_request_avg_bytes.push(r.mem_request_avg);
        response.mem_waste_bytes.push(r.mem_waste);
        response.mem_max_per_pod_bytes.push(r.mem_ws_max);
        response.psi_cpu_max_ratio.push(r.psi_cpu_max);
        response.psi_mem_max_ratio.push(r.psi_mem_max);
    }
    Ok(Json(response))
}
