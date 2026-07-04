//! ClickHouse row types. Field names must match column names; `u32` timestamps
//! map to DateTime, `Vec<(String, String)>` maps to Map(String, String).

use clickhouse::Row;
use serde::Serialize;

#[derive(Row, Serialize, Debug)]
pub struct PodUsageRow {
    pub ts: u32,
    /// Wall-clock seconds this sample covers (since the previous sample).
    /// Metric × sample_secs = exact resource-seconds, immune to partial
    /// rollup windows.
    pub sample_secs: f32,
    pub node: String,
    pub pod_uid: String,
    pub namespace: String,
    pub pod_name: String,
    pub workload_kind: String,
    pub workload_name: String,
    pub qos: String,
    pub cpu_usage_millicores: f32,
    pub cpu_request_millicores: u32,
    pub cpu_limit_millicores: u32,
    pub cpu_throttled_ratio: f32,
    pub mem_working_set_bytes: u64,
    pub mem_current_bytes: u64,
    pub mem_request_bytes: u64,
    pub mem_limit_bytes: u64,
    pub psi_cpu_some_ratio: f32,
    pub psi_mem_some_ratio: f32,
}

#[derive(Row, Serialize, Debug)]
pub struct NodeUsageRow {
    pub ts: u32,
    pub sample_secs: f32,
    pub node: String,
    pub cpu_used_millicores: f32,
    pub cpu_capacity_millicores: u32,
    pub mem_used_bytes: u64,
    pub mem_total_bytes: u64,
    pub pod_count: u32,
}

#[derive(Row, Serialize, Debug)]
pub struct PodMetaRow {
    pub pod_uid: String,
    pub namespace: String,
    pub pod_name: String,
    pub node: String,
    pub labels: Vec<(String, String)>,
    pub workload_kind: String,
    pub workload_name: String,
    pub updated_at: u32,
}
