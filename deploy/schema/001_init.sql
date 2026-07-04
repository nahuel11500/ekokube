-- ekokube ClickHouse schema — fact and dimension tables.
-- Applied automatically in dev (docker-entrypoint-initdb.d) and by the
-- Helm migration job in-cluster. Database creation lives in the migration
-- job (Replicated engine in HA mode) / 000_database.sql in dev.

-- Raw per-pod samples (one row per pod per scrape interval). Short TTL;
-- long-range queries use the rollups in 002_rollups.sql.
CREATE TABLE IF NOT EXISTS ekokube.pod_usage
(
    ts                      DateTime CODEC(Delta, ZSTD),
    -- wall-clock seconds this sample covers; metric * sample_secs = exact
    -- resource-seconds (the unit rollups and chargeback are built on)
    sample_secs             Float32 CODEC(Gorilla, ZSTD),
    node                    LowCardinality(String),
    pod_uid                 String CODEC(ZSTD),
    namespace               LowCardinality(String),
    pod_name                String CODEC(ZSTD),
    workload_kind           LowCardinality(String),
    workload_name           LowCardinality(String),
    qos                     LowCardinality(String),
    cpu_usage_millicores    Float32 CODEC(Gorilla, ZSTD),
    cpu_request_millicores  UInt32 CODEC(T64, ZSTD),
    cpu_limit_millicores    UInt32 CODEC(T64, ZSTD),
    cpu_throttled_ratio     Float32 CODEC(Gorilla, ZSTD),
    mem_working_set_bytes   UInt64 CODEC(T64, ZSTD),
    mem_current_bytes       UInt64 CODEC(T64, ZSTD),
    mem_request_bytes       UInt64 CODEC(T64, ZSTD),
    mem_limit_bytes         UInt64 CODEC(T64, ZSTD),
    psi_cpu_some_ratio      Float32 CODEC(Gorilla, ZSTD),
    psi_mem_some_ratio      Float32 CODEC(Gorilla, ZSTD)
)
ENGINE = MergeTree
PARTITION BY toDate(ts)
ORDER BY (namespace, workload_kind, workload_name, pod_name, ts)
TTL ts + INTERVAL 14 DAY;

-- Raw per-node samples.
CREATE TABLE IF NOT EXISTS ekokube.node_usage
(
    ts                      DateTime CODEC(Delta, ZSTD),
    sample_secs             Float32 CODEC(Gorilla, ZSTD),
    node                    LowCardinality(String),
    cpu_used_millicores     Float32 CODEC(Gorilla, ZSTD),
    cpu_capacity_millicores UInt32 CODEC(T64, ZSTD),
    mem_used_bytes          UInt64 CODEC(T64, ZSTD),
    mem_total_bytes         UInt64 CODEC(T64, ZSTD),
    pod_count               UInt32 CODEC(T64, ZSTD)
)
ENGINE = MergeTree
PARTITION BY toDate(ts)
ORDER BY (node, ts)
TTL ts + INTERVAL 14 DAY;

-- Pod dimension: labels and workload identity, written by agents on change.
-- Joined at query time for label-based tenancy.
CREATE TABLE IF NOT EXISTS ekokube.pod_meta
(
    pod_uid       String,
    namespace     LowCardinality(String),
    pod_name      String,
    node          LowCardinality(String),
    labels        Map(LowCardinality(String), String),
    workload_kind LowCardinality(String),
    workload_name LowCardinality(String),
    updated_at    DateTime
)
ENGINE = ReplacingMergeTree(updated_at)
ORDER BY pod_uid;

-- Namespace dimension, synced by ekokube-server from the API server.
CREATE TABLE IF NOT EXISTS ekokube.namespaces
(
    name       LowCardinality(String),
    labels     Map(LowCardinality(String), String),
    updated_at DateTime
)
ENGINE = ReplacingMergeTree(updated_at)
ORDER BY name;

-- Node dimension, synced by ekokube-server from the API server.
CREATE TABLE IF NOT EXISTS ekokube.nodes
(
    name                     LowCardinality(String),
    labels                   Map(LowCardinality(String), String),
    cpu_allocatable_millicores UInt32,
    mem_allocatable_bytes    UInt64,
    updated_at               DateTime
)
ENGINE = ReplacingMergeTree(updated_at)
ORDER BY name;

-- Tenant mapping rules, managed from the UI as one ordered rule set
-- (stored as a JSON document; the server compiles it into SQL at query time,
-- so tenancy is configurable and applies retroactively to historical data).
CREATE TABLE IF NOT EXISTS ekokube.tenant_config
(
    key        String,      -- always 'default' for now
    rules_json String,
    updated_at DateTime
)
ENGINE = ReplacingMergeTree(updated_at)
ORDER BY key;
