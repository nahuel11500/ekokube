-- Rollups: hourly and daily aggregates per pod. All long-range UI queries
-- read these instead of raw samples.
--
-- Everything is based on resource-seconds (metric * sample_secs), so sums are
-- exact regardless of partial hours or pod lifetimes:
--   avg concurrent usage over a window = sum(*_secs) / window_seconds
--   core-hours = sum(cpu_core_secs) / 3600

CREATE TABLE IF NOT EXISTS ekokube.pod_usage_1h
(
    hour                  DateTime CODEC(Delta, ZSTD),
    namespace             LowCardinality(String),
    workload_kind         LowCardinality(String),
    workload_name         LowCardinality(String),
    pod_name              String CODEC(ZSTD),
    pod_uid               String CODEC(ZSTD),
    node                  LowCardinality(String),
    qos                   LowCardinality(String),
    samples               SimpleAggregateFunction(sum, UInt64),
    secs                  SimpleAggregateFunction(sum, Float64),
    cpu_core_secs         SimpleAggregateFunction(sum, Float64),
    cpu_request_core_secs SimpleAggregateFunction(sum, Float64),
    cpu_limit_core_secs   SimpleAggregateFunction(sum, Float64),
    cpu_throttled_secs    SimpleAggregateFunction(sum, Float64),
    cpu_throttled_max     SimpleAggregateFunction(max, Float32),
    cpu_usage_max         SimpleAggregateFunction(max, Float32),
    cpu_usage_p95         AggregateFunction(quantileTDigest(0.95), Float32),
    mem_ws_byte_secs      SimpleAggregateFunction(sum, Float64),
    mem_request_byte_secs SimpleAggregateFunction(sum, Float64),
    mem_limit_byte_secs   SimpleAggregateFunction(sum, Float64),
    mem_ws_max            SimpleAggregateFunction(max, UInt64),
    mem_ws_p95            AggregateFunction(quantileTDigest(0.95), Float32),
    psi_cpu_some_max      SimpleAggregateFunction(max, Float32),
    psi_mem_some_max      SimpleAggregateFunction(max, Float32)
)
ENGINE = AggregatingMergeTree
PARTITION BY toYYYYMM(hour)
ORDER BY (namespace, workload_kind, workload_name, pod_name, pod_uid, hour)
TTL hour + INTERVAL 400 DAY;

CREATE MATERIALIZED VIEW IF NOT EXISTS ekokube.pod_usage_1h_mv
TO ekokube.pod_usage_1h AS
SELECT
    toStartOfHour(ts)                                   AS hour,
    namespace,
    workload_kind,
    workload_name,
    pod_name,
    pod_uid,
    node,
    qos,
    count()                                             AS samples,
    sum(toFloat64(sample_secs))                         AS secs,
    sum(cpu_usage_millicores * sample_secs) / 1000      AS cpu_core_secs,
    sum(cpu_request_millicores * sample_secs) / 1000    AS cpu_request_core_secs,
    sum(cpu_limit_millicores * sample_secs) / 1000      AS cpu_limit_core_secs,
    sum(cpu_throttled_ratio * sample_secs)              AS cpu_throttled_secs,
    max(cpu_throttled_ratio)                            AS cpu_throttled_max,
    max(cpu_usage_millicores)                           AS cpu_usage_max,
    quantileTDigestState(0.95)(cpu_usage_millicores)    AS cpu_usage_p95,
    sum(mem_working_set_bytes * sample_secs)            AS mem_ws_byte_secs,
    sum(mem_request_bytes * sample_secs)                AS mem_request_byte_secs,
    sum(mem_limit_bytes * sample_secs)                  AS mem_limit_byte_secs,
    max(mem_working_set_bytes)                          AS mem_ws_max,
    quantileTDigestState(0.95)(toFloat32(mem_working_set_bytes)) AS mem_ws_p95,
    max(psi_cpu_some_ratio)                             AS psi_cpu_some_max,
    max(psi_mem_some_ratio)                             AS psi_mem_some_max
FROM ekokube.pod_usage
GROUP BY hour, namespace, workload_kind, workload_name, pod_name, pod_uid, node, qos;

CREATE TABLE IF NOT EXISTS ekokube.pod_usage_1d
(
    day                   Date,
    namespace             LowCardinality(String),
    workload_kind         LowCardinality(String),
    workload_name         LowCardinality(String),
    pod_name              String CODEC(ZSTD),
    pod_uid               String CODEC(ZSTD),
    node                  LowCardinality(String),
    qos                   LowCardinality(String),
    samples               SimpleAggregateFunction(sum, UInt64),
    secs                  SimpleAggregateFunction(sum, Float64),
    cpu_core_secs         SimpleAggregateFunction(sum, Float64),
    cpu_request_core_secs SimpleAggregateFunction(sum, Float64),
    cpu_limit_core_secs   SimpleAggregateFunction(sum, Float64),
    cpu_throttled_secs    SimpleAggregateFunction(sum, Float64),
    cpu_throttled_max     SimpleAggregateFunction(max, Float32),
    cpu_usage_max         SimpleAggregateFunction(max, Float32),
    cpu_usage_p95         AggregateFunction(quantileTDigest(0.95), Float32),
    mem_ws_byte_secs      SimpleAggregateFunction(sum, Float64),
    mem_request_byte_secs SimpleAggregateFunction(sum, Float64),
    mem_limit_byte_secs   SimpleAggregateFunction(sum, Float64),
    mem_ws_max            SimpleAggregateFunction(max, UInt64),
    mem_ws_p95            AggregateFunction(quantileTDigest(0.95), Float32),
    psi_cpu_some_max      SimpleAggregateFunction(max, Float32),
    psi_mem_some_max      SimpleAggregateFunction(max, Float32)
)
ENGINE = AggregatingMergeTree
PARTITION BY toYYYYMM(day)
ORDER BY (namespace, workload_kind, workload_name, pod_name, pod_uid, day)
TTL day + INTERVAL 1100 DAY;

CREATE MATERIALIZED VIEW IF NOT EXISTS ekokube.pod_usage_1d_mv
TO ekokube.pod_usage_1d AS
SELECT
    toDate(ts)                                          AS day,
    namespace,
    workload_kind,
    workload_name,
    pod_name,
    pod_uid,
    node,
    qos,
    count()                                             AS samples,
    sum(toFloat64(sample_secs))                         AS secs,
    sum(cpu_usage_millicores * sample_secs) / 1000      AS cpu_core_secs,
    sum(cpu_request_millicores * sample_secs) / 1000    AS cpu_request_core_secs,
    sum(cpu_limit_millicores * sample_secs) / 1000      AS cpu_limit_core_secs,
    sum(cpu_throttled_ratio * sample_secs)              AS cpu_throttled_secs,
    max(cpu_throttled_ratio)                            AS cpu_throttled_max,
    max(cpu_usage_millicores)                           AS cpu_usage_max,
    quantileTDigestState(0.95)(cpu_usage_millicores)    AS cpu_usage_p95,
    sum(mem_working_set_bytes * sample_secs)            AS mem_ws_byte_secs,
    sum(mem_request_bytes * sample_secs)                AS mem_request_byte_secs,
    sum(mem_limit_bytes * sample_secs)                  AS mem_limit_byte_secs,
    max(mem_working_set_bytes)                          AS mem_ws_max,
    quantileTDigestState(0.95)(toFloat32(mem_working_set_bytes)) AS mem_ws_p95,
    max(psi_cpu_some_ratio)                             AS psi_cpu_some_max,
    max(psi_mem_some_ratio)                             AS psi_mem_some_max
FROM ekokube.pod_usage
GROUP BY day, namespace, workload_kind, workload_name, pod_name, pod_uid, node, qos;

-- Node rollup (hourly) for capacity/saturation views.
CREATE TABLE IF NOT EXISTS ekokube.node_usage_1h
(
    hour               DateTime CODEC(Delta, ZSTD),
    node               LowCardinality(String),
    samples            SimpleAggregateFunction(sum, UInt64),
    secs               SimpleAggregateFunction(sum, Float64),
    cpu_used_core_secs SimpleAggregateFunction(sum, Float64),
    cpu_used_max       SimpleAggregateFunction(max, Float32),
    cpu_capacity_max   SimpleAggregateFunction(max, UInt32),
    mem_used_byte_secs SimpleAggregateFunction(sum, Float64),
    mem_used_max       SimpleAggregateFunction(max, UInt64),
    mem_total_max      SimpleAggregateFunction(max, UInt64),
    pod_count_max      SimpleAggregateFunction(max, UInt32)
)
ENGINE = AggregatingMergeTree
PARTITION BY toYYYYMM(hour)
ORDER BY (node, hour)
TTL hour + INTERVAL 400 DAY;

CREATE MATERIALIZED VIEW IF NOT EXISTS ekokube.node_usage_1h_mv
TO ekokube.node_usage_1h AS
SELECT
    toStartOfHour(ts)                              AS hour,
    node,
    count()                                        AS samples,
    sum(toFloat64(sample_secs))                    AS secs,
    sum(cpu_used_millicores * sample_secs) / 1000  AS cpu_used_core_secs,
    max(cpu_used_millicores)                       AS cpu_used_max,
    max(cpu_capacity_millicores)                   AS cpu_capacity_max,
    sum(mem_used_bytes * sample_secs)              AS mem_used_byte_secs,
    max(mem_used_bytes)                            AS mem_used_max,
    max(mem_total_bytes)                           AS mem_total_max,
    max(pod_count)                                 AS pod_count_max
FROM ekokube.node_usage
GROUP BY hour, node;
