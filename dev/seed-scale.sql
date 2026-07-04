-- Synthetic scale dataset, written straight into the hourly rollup:
--   500 nodes x 200k pod series x 30 days hourly = 144M rows
--   400 namespaces, 20k workloads.
-- Usage/requests are deterministic functions of the series id so waste
-- rankings are stable. initializeAggregation builds the tdigest states
-- per row (no GROUP BY over 144M rows needed).
--
-- Apply with: clickhouse-client --multiquery < dev/seed-scale.sql  (takes a few minutes)

INSERT INTO ekokube.pod_usage_1h
SELECT
    toStartOfHour(now() - toIntervalHour(h))                    AS hour,
    concat('ns-', toString(s % 400))                            AS namespace,
    'Deployment'                                                AS workload_kind,
    concat('wl-', toString(s % 20000))                          AS workload_name,
    concat('pod-', toString(s))                                 AS pod_name,
    concat('uid-', toString(s))                                 AS pod_uid,
    concat('node-', toString(s % 500))                          AS node,
    'Burstable'                                                 AS qos,
    240                                                         AS samples,
    3600.0                                                      AS secs,
    (50 + s % 950) * 3.6 * (1 + 0.2 * sin(h / 24.0))            AS cpu_core_secs,
    (100 + (s % 19) * 100) * 3.6                                AS cpu_request_core_secs,
    (100 + (s % 19) * 200) * 3.6                                AS cpu_limit_core_secs,
    if(s % 37 = 0, 360.0, 0.0)                                  AS cpu_throttled_secs,
    if(s % 37 = 0, 0.8, 0.0)                                    AS cpu_throttled_max,
    toFloat32((50 + s % 950) * 1.3)                             AS cpu_usage_max,
    initializeAggregation('quantileTDigestState(0.95)', toFloat32((50 + s % 950) * 1.2)) AS cpu_usage_p95,
    ((s % 64 + 1) * 33554432) * 3600.0                          AS mem_ws_byte_secs,
    ((s % 64 + 4) * 33554432) * 3600.0                          AS mem_request_byte_secs,
    ((s % 64 + 8) * 33554432) * 3600.0                          AS mem_limit_byte_secs,
    toUInt64((s % 64 + 2) * 33554432)                           AS mem_ws_max,
    initializeAggregation('quantileTDigestState(0.95)', toFloat32((s % 64 + 1) * 33554432)) AS mem_ws_p95,
    toFloat32(0)                                                AS psi_cpu_some_max,
    toFloat32(0)                                                AS psi_mem_some_max
FROM
(
    SELECT number % 200000 AS s, intDiv(number, 200000) AS h
    FROM numbers(200000 * 720)
);

-- Daily rollup: same series, one row per day (200k x 30 = 6M rows). Long-range
-- queries (>3d) read this instead of the hourly table.
INSERT INTO ekokube.pod_usage_1d
SELECT
    toDate(now() - toIntervalDay(d))                            AS day,
    concat('ns-', toString(s % 400))                            AS namespace,
    'Deployment'                                                AS workload_kind,
    concat('wl-', toString(s % 20000))                          AS workload_name,
    concat('pod-', toString(s))                                 AS pod_name,
    concat('uid-', toString(s))                                 AS pod_uid,
    concat('node-', toString(s % 500))                          AS node,
    'Burstable'                                                 AS qos,
    5760                                                        AS samples,
    86400.0                                                     AS secs,
    (50 + s % 950) * 86.4                                       AS cpu_core_secs,
    (100 + (s % 19) * 100) * 86.4                               AS cpu_request_core_secs,
    (100 + (s % 19) * 200) * 86.4                               AS cpu_limit_core_secs,
    if(s % 37 = 0, 8640.0, 0.0)                                 AS cpu_throttled_secs,
    if(s % 37 = 0, 0.8, 0.0)                                    AS cpu_throttled_max,
    toFloat32((50 + s % 950) * 1.3)                             AS cpu_usage_max,
    initializeAggregation('quantileTDigestState(0.95)', toFloat32((50 + s % 950) * 1.2)) AS cpu_usage_p95,
    ((s % 64 + 1) * 33554432) * 86400.0                         AS mem_ws_byte_secs,
    ((s % 64 + 4) * 33554432) * 86400.0                         AS mem_request_byte_secs,
    ((s % 64 + 8) * 33554432) * 86400.0                         AS mem_limit_byte_secs,
    toUInt64((s % 64 + 2) * 33554432)                           AS mem_ws_max,
    initializeAggregation('quantileTDigestState(0.95)', toFloat32((s % 64 + 1) * 33554432)) AS mem_ws_p95,
    toFloat32(0)                                                AS psi_cpu_some_max,
    toFloat32(0)                                                AS psi_mem_some_max
FROM
(
    SELECT number % 200000 AS s, intDiv(number, 200000) AS d
    FROM numbers(200000 * 30)
);

-- Matching node rollup (500 nodes, 30 days hourly).
INSERT INTO ekokube.node_usage_1h
SELECT
    toStartOfHour(now() - toIntervalHour(h)) AS hour,
    concat('node-', toString(n))             AS node,
    240                                      AS samples,
    3600.0                                   AS secs,
    (20000 + (n % 40) * 500) * 3.6           AS cpu_used_core_secs,
    toFloat32(30000 + (n % 40) * 500)        AS cpu_used_max,
    64000                                    AS cpu_capacity_max,
    2.0e11 * 3600                            AS mem_used_byte_secs,
    toUInt64(2.5e11)                         AS mem_used_max,
    toUInt64(2.7e11)                         AS mem_total_max,
    400                                      AS pod_count_max
FROM
(
    SELECT number % 500 AS n, intDiv(number, 500) AS h
    FROM numbers(500 * 720)
);
