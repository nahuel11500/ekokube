//! Per-namespace consumption and waste over a period (from hourly rollups).

use axum::extract::{Query, State};
use axum::Json;
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState, RangeQuery};

const SORT_COLUMNS: &[&str] = &[
    "cpu_usage_avg",
    "cpu_request_avg",
    "cpu_waste",
    "cpu_core_hours",
    "mem_usage_avg",
    "mem_request_avg",
    "mem_waste",
    "mem_gib_hours",
    "pods",
    "namespace",
];

#[derive(Row, Deserialize)]
struct NamespaceStat {
    namespace: String,
    pods: u64,
    cpu_usage_avg: f64,
    cpu_request_avg: f64,
    cpu_waste: f64,
    cpu_core_hours: f64,
    mem_usage_avg: f64,
    mem_request_avg: f64,
    mem_waste: f64,
    mem_gib_hours: f64,
}

#[derive(Serialize, Default)]
pub struct NamespacesResponse {
    namespace: Vec<String>,
    pods: Vec<u64>,
    cpu_usage_avg_millicores: Vec<f64>,
    cpu_request_avg_millicores: Vec<f64>,
    cpu_waste_millicores: Vec<f64>,
    cpu_core_hours: Vec<f64>,
    mem_usage_avg_bytes: Vec<f64>,
    mem_request_avg_bytes: Vec<f64>,
    mem_waste_bytes: Vec<f64>,
    mem_gib_hours: Vec<f64>,
    has_more: bool,
}

pub async fn handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<NamespacesResponse>, ApiError> {
    let (from, to) = query.pod_range();
    let window = query.window_secs();
    let sort = query.sort_sql(SORT_COLUMNS, "cpu_usage_avg");
    let limit = query.limit();
    let (table, _) = query.pod_source();
    let where_clause = query.pod_where();

    let sql = format!(
        "SELECT namespace, \
                uniqExact(pod_uid) AS pods, \
                sum(cpu_core_secs)*1000/{window} AS cpu_usage_avg, \
                sum(cpu_request_core_secs)*1000/{window} AS cpu_request_avg, \
                cpu_request_avg - cpu_usage_avg AS cpu_waste, \
                sum(cpu_core_secs)/3600 AS cpu_core_hours, \
                sum(mem_ws_byte_secs)/{window} AS mem_usage_avg, \
                sum(mem_request_byte_secs)/{window} AS mem_request_avg, \
                mem_request_avg - mem_usage_avg AS mem_waste, \
                sum(mem_ws_byte_secs)/3600/1073741824 AS mem_gib_hours \
         FROM {table} \
         WHERE {where_clause} \
         GROUP BY namespace \
         ORDER BY {sort} \
         LIMIT ? OFFSET ?"
    );
    let mut rows: Vec<NamespaceStat> = state
        .ch
        .query(&sql)
        .bind(from)
        .bind(to)
        .bind(limit + 1)
        .bind(query.offset())
        .fetch_all()
        .await?;

    let has_more = rows.len() as u32 > limit;
    rows.truncate(limit as usize);

    let mut response = NamespacesResponse {
        has_more,
        ..Default::default()
    };
    for r in rows {
        response.namespace.push(r.namespace);
        response.pods.push(r.pods);
        response.cpu_usage_avg_millicores.push(r.cpu_usage_avg);
        response.cpu_request_avg_millicores.push(r.cpu_request_avg);
        response.cpu_waste_millicores.push(r.cpu_waste);
        response.cpu_core_hours.push(r.cpu_core_hours);
        response.mem_usage_avg_bytes.push(r.mem_usage_avg);
        response.mem_request_avg_bytes.push(r.mem_request_avg);
        response.mem_waste_bytes.push(r.mem_waste);
        response.mem_gib_hours.push(r.mem_gib_hours);
    }
    Ok(Json(response))
}
