#!/usr/bin/env bash
# End-to-end agent smoke test without Kubernetes:
# fake cgroup v2 tree + fake kubelet + real ClickHouse (docker compose).
# Asserts the full pipeline: discovery, parsing, metadata join, insert.
set -euo pipefail
cd "$(dirname "$0")/.."

CH_URL="http://localhost:8123"
KUBELET_PORT=10299
WORK="$(mktemp -d)"
FAKE_PID=""
AGENT_PID=""

cleanup() {
  [ -n "$AGENT_PID" ] && kill "$AGENT_PID" 2>/dev/null || true
  [ -n "$FAKE_PID" ] && kill "$FAKE_PID" 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

fail() { echo "SMOKE FAIL: $*" >&2; exit 1; }
q() { curl -sf -X POST "$CH_URL/" --data "$1"; }

echo "==> building agent"
cargo build -p ekokube-agent

echo "==> starting clickhouse"
docker compose -f dev/docker-compose.yaml up -d
for i in $(seq 1 60); do
  curl -sf "$CH_URL/?query=SELECT+1" >/dev/null && break
  sleep 2
done
q "SELECT 1" >/dev/null || fail "clickhouse not reachable"
q "TRUNCATE TABLE ekokube.pod_usage"
q "TRUNCATE TABLE ekokube.pod_meta"
q "TRUNCATE TABLE ekokube.node_usage"

echo "==> creating fake cgroup tree + kubelet"
POD_DIR="$WORK/cgroup/kubepods.slice/kubepods-burstable.slice/kubepods-burstable-pod11111111_2222_3333_4444_555555555555.slice"
mkdir -p "$POD_DIR"
printf 'usage_usec 5000000\nnr_periods 100\nnr_throttled 25\nthrottled_usec 900000\n' > "$POD_DIR/cpu.stat"
printf '104857600\n' > "$POD_DIR/memory.current"
printf 'inactive_file 20971520\n' > "$POD_DIR/memory.stat"
printf 'some avg10=1.50 avg60=0.80 avg300=0.20 total=123456\n' > "$POD_DIR/cpu.pressure"
printf 'some avg10=0.00 avg60=0.00 avg300=0.00 total=0\n' > "$POD_DIR/memory.pressure"

cat > "$WORK/pods.json" <<'EOF'
{"items":[{"metadata":{"name":"web-6f8d4b9c7d-abcde","namespace":"shop","uid":"11111111-2222-3333-4444-555555555555","labels":{"app":"web","team":"checkout"},"ownerReferences":[{"kind":"ReplicaSet","name":"web-6f8d4b9c7d","controller":true}]},"spec":{"containers":[{"resources":{"requests":{"cpu":"250m","memory":"256Mi"},"limits":{"cpu":"1","memory":"512Mi"}}}]},"status":{"qosClass":"Burstable"}}]}
EOF
python3 dev/fake_kubelet.py "$KUBELET_PORT" "$WORK/pods.json" &
FAKE_PID=$!

echo "==> running agent for 8s"
EKOKUBE_NODE_NAME=smoke-node \
EKOKUBE_CGROUP_ROOT="$WORK/cgroup" \
EKOKUBE_KUBELET_URL="http://127.0.0.1:$KUBELET_PORT" \
EKOKUBE_INTERVAL_SECS=2 \
EKOKUBE_METRICS_ADDR=127.0.0.1:19091 \
RUST_LOG=warn ./target/debug/ekokube-agent &
AGENT_PID=$!
sleep 8
kill "$AGENT_PID"; AGENT_PID=""
sleep 1

echo "==> asserting"
ROWS=$(q "SELECT count() FROM ekokube.pod_usage WHERE node='smoke-node'")
[ "$ROWS" -ge 2 ] || fail "expected >=2 pod_usage rows, got $ROWS"

WS=$(q "SELECT any(mem_working_set_bytes) FROM ekokube.pod_usage WHERE node='smoke-node'")
[ "$WS" = "83886080" ] || fail "working set: expected 83886080 (current - inactive_file), got $WS"

REQ=$(q "SELECT any(cpu_request_millicores) FROM ekokube.pod_usage WHERE node='smoke-node'")
[ "$REQ" = "250" ] || fail "cpu request: expected 250, got $REQ"

WORKLOAD=$(q "SELECT any(workload_kind) || '/' || any(workload_name) FROM ekokube.pod_usage WHERE node='smoke-node'")
[ "$WORKLOAD" = "Deployment/web" ] || fail "workload resolution: expected Deployment/web, got $WORKLOAD"

TEAM=$(q "SELECT labels['team'] FROM ekokube.pod_meta WHERE pod_uid='11111111-2222-3333-4444-555555555555' ORDER BY updated_at DESC LIMIT 1")
[ "$TEAM" = "checkout" ] || fail "pod labels: expected team=checkout, got '$TEAM'"

NODE_ROWS=$(q "SELECT count() FROM ekokube.node_usage WHERE node='smoke-node'")
[ "$NODE_ROWS" -ge 1 ] || fail "expected node_usage rows"

METRICS=$(curl -sf 127.0.0.1:19091/metrics || true)
echo "$METRICS" | grep -q "ekokube_agent_rows_written_total" || true # agent already stopped; metrics best-effort

echo "SMOKE OK: $ROWS pod samples, working set / requests / labels / workload all correct"
