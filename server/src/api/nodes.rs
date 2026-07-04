//! Per-node saturation and bin-packing headroom (usage + requested vs allocatable).

use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::Json;
use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState, RangeQuery};

#[derive(Row, Deserialize)]
struct NodeStat {
    node: String,
    cpu_used_avg: f64,
    cpu_used_max: f32,
    cpu_capacity: u32,
    mem_used_avg: f64,
    mem_used_max: u64,
    mem_total: u64,
    pods: u32,
}

#[derive(Row, Deserialize)]
struct NodeRequested {
    node: String,
    cpu_requested_avg: f64,
    mem_requested_avg: f64,
}

#[derive(Row, Deserialize)]
struct NodeAllocatable {
    name: String,
    cpu_allocatable_millicores: u32,
    mem_allocatable_bytes: u64,
}

#[derive(Serialize, Default)]
pub struct NodesResponse {
    node: Vec<String>,
    pods: Vec<u32>,
    cpu_used_avg_millicores: Vec<f64>,
    cpu_used_max_millicores: Vec<f32>,
    cpu_capacity_millicores: Vec<u32>,
    cpu_allocatable_millicores: Vec<u32>,
    cpu_requested_avg_millicores: Vec<f64>,
    mem_used_avg_bytes: Vec<f64>,
    mem_used_max_bytes: Vec<u64>,
    mem_total_bytes: Vec<u64>,
    mem_allocatable_bytes: Vec<u64>,
    mem_requested_avg_bytes: Vec<f64>,
}

pub async fn handler(
    State(state): State<AppState>,
    Query(query): Query<RangeQuery>,
) -> Result<Json<NodesResponse>, ApiError> {
    // Node usage only has an hourly rollup; align its own bounds to the hour.
    let (nfrom, nto) = query.range();
    let nfrom = nfrom / 3600 * 3600;
    let nto = nto.max(nfrom + 60);

    // Per-node averages are over observed time (secs), not the query window,
    // so a node that joined mid-window still shows its true utilization.
    let sql = "SELECT node, \
                sum(cpu_used_core_secs)*1000/sum(secs) AS cpu_used_avg, \
                max(cpu_used_max) AS cpu_used_max, \
                max(cpu_capacity_max) AS cpu_capacity, \
                sum(mem_used_byte_secs)/sum(secs) AS mem_used_avg, \
                max(mem_used_max) AS mem_used_max, \
                max(mem_total_max) AS mem_total, \
                max(pod_count_max) AS pods \
         FROM node_usage_1h WHERE hour >= toDateTime(?) AND hour < toDateTime(?) \
         GROUP BY node ORDER BY node";
    let stats: Vec<NodeStat> = state
        .ch
        .query(sql)
        .bind(nfrom)
        .bind(nto)
        .fetch_all()
        .await?;

    // Requested totals per node come from the pod rollup (daily for long ranges).
    let (from, to) = query.pod_range();
    let window = query.window_secs();
    let (table, _) = query.pod_source();
    let where_clause = query.pod_where();
    let requested_sql = format!(
        "SELECT node, \
                sum(cpu_request_core_secs)*1000/{window} AS cpu_requested_avg, \
                sum(mem_request_byte_secs)/{window} AS mem_requested_avg \
         FROM {table} WHERE {where_clause} \
         GROUP BY node"
    );
    let requested: HashMap<String, NodeRequested> = state
        .ch
        .query(&requested_sql)
        .bind(from)
        .bind(to)
        .fetch_all::<NodeRequested>()
        .await?
        .into_iter()
        .map(|r| (r.node.clone(), r))
        .collect();

    let allocatable: HashMap<String, NodeAllocatable> = state
        .ch
        .query(
            "SELECT name, cpu_allocatable_millicores, mem_allocatable_bytes \
             FROM nodes FINAL",
        )
        .fetch_all::<NodeAllocatable>()
        .await?
        .into_iter()
        .map(|r| (r.name.clone(), r))
        .collect();

    let mut response = NodesResponse::default();
    for s in stats {
        let req = requested.get(&s.node);
        let alloc = allocatable.get(&s.node);
        response.node.push(s.node.clone());
        response.pods.push(s.pods);
        response.cpu_used_avg_millicores.push(s.cpu_used_avg);
        response.cpu_used_max_millicores.push(s.cpu_used_max);
        response.cpu_capacity_millicores.push(s.cpu_capacity);
        response
            .cpu_allocatable_millicores
            .push(alloc.map(|a| a.cpu_allocatable_millicores).unwrap_or(0));
        response
            .cpu_requested_avg_millicores
            .push(req.map(|r| r.cpu_requested_avg).unwrap_or(0.0));
        response.mem_used_avg_bytes.push(s.mem_used_avg);
        response.mem_used_max_bytes.push(s.mem_used_max);
        response.mem_total_bytes.push(s.mem_total);
        response
            .mem_allocatable_bytes
            .push(alloc.map(|a| a.mem_allocatable_bytes).unwrap_or(0));
        response
            .mem_requested_avg_bytes
            .push(req.map(|r| r.mem_requested_avg).unwrap_or(0.0));
    }
    Ok(Json(response))
}
