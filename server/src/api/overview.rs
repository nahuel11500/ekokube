//! Cluster-wide time series (usage vs requests vs capacity) + current summary.

use axum::extract::{Query, State};
use axum::Json;
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::{choose_bucket, ApiError, AppState, RangeQuery};

#[derive(Row, Deserialize)]
struct SeriesPoint {
    t: u32,
    cpu_usage: f64,
    cpu_request: f64,
    mem_ws: f64,
    mem_request: f64,
}

#[derive(Row, Deserialize)]
struct NodePoint {
    t: u32,
    cpu_used: f64,
    cpu_capacity: f64,
    mem_used: f64,
    mem_total: f64,
}

#[derive(Row, Deserialize)]
struct Summary {
    nodes: u64,
    pods: u64,
    namespaces: u64,
}

/// Columnar response: parallel arrays indexed by bucket.
#[derive(Serialize, Default)]
pub struct OverviewResponse {
    step: u32,
    ts: Vec<u32>,
    cpu_usage_millicores: Vec<f64>,
    cpu_request_millicores: Vec<f64>,
    mem_working_set_bytes: Vec<f64>,
    mem_request_bytes: Vec<f64>,
    node_ts: Vec<u32>,
    cpu_used_millicores: Vec<f64>,
    cpu_capacity_millicores: Vec<f64>,
    mem_used_bytes: Vec<f64>,
    mem_total_bytes: Vec<f64>,
    nodes: u64,
    pods: u64,
    namespaces: u64,
}

pub async fn handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<OverviewResponse>, ApiError> {
    let (from, to) = query.range();
    let (step, raw) = choose_bucket(from, to);
    // Beyond ~10 days, read the daily rollup (one point per day) so a 30-day
    // query scans ~200k×30 rows instead of ~200k×720.
    let daily = to.saturating_sub(from) > 10 * 24 * 3600;
    let step = if daily { 24 * 3600 } else { step };
    // Align to bucket boundaries and exclude the in-progress bucket, so every
    // plotted point covers a full bucket (no misleading edge dips).
    let from = from / step * step;
    let to = (to / step * step).max(from + step);

    // sum(resource_secs) / bucket_width = exact average concurrent usage.
    let pod_sql = if daily {
        format!(
            "SELECT toDateTime(day) AS t, \
                    sum(cpu_core_secs)*1000/{step} AS cpu_usage, \
                    sum(cpu_request_core_secs)*1000/{step} AS cpu_request, \
                    sum(mem_ws_byte_secs)/{step} AS mem_ws, \
                    sum(mem_request_byte_secs)/{step} AS mem_request \
             FROM pod_usage_1d \
             WHERE day >= toDate(toDateTime(?)) AND day < toDate(toDateTime(?)) \
             GROUP BY t ORDER BY t"
        )
    } else if raw {
        format!(
            "SELECT toStartOfInterval(ts, INTERVAL {step} SECOND) AS t, \
                    sum(cpu_usage_millicores * sample_secs)/{step} AS cpu_usage, \
                    sum(cpu_request_millicores * sample_secs)/{step} AS cpu_request, \
                    sum(mem_working_set_bytes * sample_secs)/{step} AS mem_ws, \
                    sum(mem_request_bytes * sample_secs)/{step} AS mem_request \
             FROM pod_usage WHERE ts >= ? AND ts < ? GROUP BY t ORDER BY t"
        )
    } else {
        format!(
            "SELECT toStartOfInterval(hour, INTERVAL {step} SECOND) AS t, \
                    sum(cpu_core_secs)*1000/{step} AS cpu_usage, \
                    sum(cpu_request_core_secs)*1000/{step} AS cpu_request, \
                    sum(mem_ws_byte_secs)/{step} AS mem_ws, \
                    sum(mem_request_byte_secs)/{step} AS mem_request \
             FROM pod_usage_1h WHERE hour >= ? AND hour < ? GROUP BY t ORDER BY t"
        )
    };
    let pod_points: Vec<SeriesPoint> = state
        .ch
        .query(&pod_sql)
        .bind(from)
        .bind(to)
        .fetch_all()
        .await?;

    // Capacity is summed per node (max within bucket) via a subquery; node
    // count × buckets is small so this stays cheap.
    let node_sql = if raw {
        format!(
            "SELECT t, sum(used) AS cpu_used, sum(cap) AS cpu_capacity, \
                    sum(mem_u) AS mem_used, sum(mem_t) AS mem_total \
             FROM ( \
               SELECT toStartOfInterval(ts, INTERVAL {step} SECOND) AS t, node, \
                      sum(cpu_used_millicores * sample_secs)/{step} AS used, \
                      toFloat64(max(cpu_capacity_millicores)) AS cap, \
                      sum(mem_used_bytes * sample_secs)/{step} AS mem_u, \
                      toFloat64(max(mem_total_bytes)) AS mem_t \
               FROM node_usage WHERE ts >= ? AND ts < ? GROUP BY t, node \
             ) GROUP BY t ORDER BY t"
        )
    } else {
        format!(
            "SELECT t, sum(used) AS cpu_used, sum(cap) AS cpu_capacity, \
                    sum(mem_u) AS mem_used, sum(mem_t) AS mem_total \
             FROM ( \
               SELECT toStartOfInterval(hour, INTERVAL {step} SECOND) AS t, node, \
                      sum(cpu_used_core_secs)*1000/{step} AS used, \
                      toFloat64(max(cpu_capacity_max)) AS cap, \
                      sum(mem_used_byte_secs)/{step} AS mem_u, \
                      toFloat64(max(mem_total_max)) AS mem_t \
               FROM node_usage_1h WHERE hour >= ? AND hour < ? GROUP BY t, node \
             ) GROUP BY t ORDER BY t"
        )
    };
    let node_points: Vec<NodePoint> = state
        .ch
        .query(&node_sql)
        .bind(from)
        .bind(to)
        .fetch_all()
        .await?;

    let summary: Summary = state
        .ch
        .query(
            "SELECT uniqExact(node) AS nodes, uniqExact(pod_uid) AS pods, \
                    uniqExact(namespace) AS namespaces \
             FROM pod_usage WHERE ts >= now() - 300",
        )
        .fetch_one()
        .await?;

    let mut response = OverviewResponse {
        step,
        nodes: summary.nodes,
        pods: summary.pods,
        namespaces: summary.namespaces,
        ..Default::default()
    };
    for p in pod_points {
        response.ts.push(p.t);
        response.cpu_usage_millicores.push(p.cpu_usage);
        response.cpu_request_millicores.push(p.cpu_request);
        response.mem_working_set_bytes.push(p.mem_ws);
        response.mem_request_bytes.push(p.mem_request);
    }
    for p in node_points {
        response.node_ts.push(p.t);
        response.cpu_used_millicores.push(p.cpu_used);
        response.cpu_capacity_millicores.push(p.cpu_capacity);
        response.mem_used_bytes.push(p.mem_used);
        response.mem_total_bytes.push(p.mem_total);
    }
    Ok(Json(response))
}
