#!/usr/bin/env bash
# Scale/latency check against the local dev ClickHouse (docker compose):
# seeds 200k pod series x 30 days of hourly rollups if absent, then requires
# every table/chart endpoint to answer warm in < 500ms.
set -euo pipefail
cd "$(dirname "$0")/.."

CH_URL="http://localhost:8123"
PORT=18081
BUDGET_MS=500

fail() { echo "PERF FAIL: $*" >&2; exit 1; }
q() { curl -sf -X POST "$CH_URL/" --data "$1"; }

docker compose -f dev/docker-compose.yaml up -d >/dev/null
for i in $(seq 1 60); do curl -sf "$CH_URL/?query=SELECT+1" >/dev/null && break; sleep 2; done

ROWS=$(q "SELECT count() FROM ekokube.pod_usage_1h")
if [ "$ROWS" -lt 100000000 ]; then
  echo "==> seeding scale dataset (144M rollup rows, takes a few minutes)"
  docker compose -f dev/docker-compose.yaml exec -T clickhouse \
    clickhouse-client --multiquery < dev/seed-scale.sql || fail "seeding failed"
  ROWS=$(q "SELECT count() FROM ekokube.pod_usage_1h")
fi
echo "==> dataset: $ROWS hourly rollup rows"

echo "==> building + starting server"
cargo build --release -p ekokube-server
EKOKUBE_SYNC_ENABLED=false EKOKUBE_LISTEN_ADDR="127.0.0.1:$PORT" ./target/release/ekokube-server &
SERVER_PID=$!
trap 'kill $SERVER_PID 2>/dev/null || true' EXIT
sleep 1

NOW=$(date +%s)
FROM=$(( NOW - 30 * 24 * 3600 ))
ENDPOINTS=(
  "/api/overview?from=$FROM&to=$NOW"
  "/api/namespaces?from=$FROM&to=$NOW&sort_by=cpu_waste"
  "/api/workloads?from=$FROM&to=$NOW&sort_by=cpu_waste"
  "/api/nodes?from=$FROM&to=$NOW"
)

STATUS=0
for ep in "${ENDPOINTS[@]}"; do
  # first hit warms caches; the second is the measured one
  curl -sf -o /dev/null "http://127.0.0.1:$PORT$ep" || fail "$ep errored"
  T=$(curl -sf -o /dev/null -w "%{time_total}" "http://127.0.0.1:$PORT$ep")
  MS=$(python3 -c "print(round($T * 1000))")
  if [ "$MS" -le "$BUDGET_MS" ]; then
    echo "  OK   ${MS}ms  $ep"
  else
    echo "  SLOW ${MS}ms  $ep (budget ${BUDGET_MS}ms)"
    STATUS=1
  fi
done

[ "$STATUS" = 0 ] && echo "PERF OK" || fail "one or more endpoints over budget"
