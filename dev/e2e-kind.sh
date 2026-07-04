#!/usr/bin/env bash
# Full e2e on a kind cluster: build + load images, install the umbrella chart
# (operator + ClickHouse + agent + server), run load generators, assert data.
#
# Prerequisites: a kind cluster, cert-manager installed.
# Usage: dev/e2e-kind.sh [--ha] [--skip-build]
set -euo pipefail
cd "$(dirname "$0")/.."

HA=false
BUILD=true
for arg in "$@"; do
  case "$arg" in
    --ha) HA=true ;;
    --skip-build) BUILD=false ;;
  esac
done

NS=ekokube
RELEASE=ekokube
fail() { echo "E2E FAIL: $*" >&2; exit 1; }

if $BUILD; then
  echo "==> building + loading images"
  docker build -f agent/Dockerfile -t ekokube-agent:dev .
  docker build -f server/Dockerfile -t ekokube-server:dev .
  kind load docker-image ekokube-agent:dev ekokube-server:dev
fi

echo "==> installing (operator + ekokube, two releases; ha=$HA)"
HA_FLAG=""
$HA && HA_FLAG="--ha"
deploy/install.sh -n "$NS" $HA_FLAG \
  --set agent.image.tag=dev --set server.image.tag=dev \
  --set clickhouse.storage=5Gi --set clickhouse.keeper.storage=1Gi \
  --timeout 20m

echo "==> waiting for ClickHouse + server readiness"
kubectl -n "$NS" wait clickhousecluster/"$RELEASE" --for=jsonpath='{.status.readyReplicas}'="$($HA && echo 2 || echo 1)" --timeout=600s \
  || kubectl -n "$NS" get clickhousecluster "$RELEASE" -o yaml | tail -20
kubectl -n "$NS" rollout status deploy/"$RELEASE"-server --timeout=300s

echo "==> deploying load generators"
kubectl apply -f dev/load-generator.yaml
kubectl -n loadgen rollout status deploy/cpu-burner --timeout=300s
kubectl -n loadgen rollout status deploy/idle-hog --timeout=300s

echo "==> letting the agent collect for 90s"
sleep 90

echo "==> asserting via the API"
kubectl -n "$NS" port-forward svc/"$RELEASE"-server 18080:80 >/dev/null 2>&1 &
PF_PID=$!
trap 'kill $PF_PID 2>/dev/null || true' EXIT
sleep 2

FROM=$(( $(date +%s) - 3600 ))
WORKLOADS=$(curl -sf "localhost:18080/api/workloads?from=$FROM&limit=100")

python3 - "$WORKLOADS" <<'EOF'
import json, sys
d = json.loads(sys.argv[1])
rows = {name: i for i, name in enumerate(d["workload_name"])}
assert "cpu-burner" in rows, f"cpu-burner not found in {list(rows)}"
assert "idle-hog" in rows, f"idle-hog not found in {list(rows)}"
burner = rows["cpu-burner"]
# spin loop pinned at its 300m limit -> p95 near the limit, throttled
p95 = d["cpu_p95_per_pod_millicores"][burner]
throttle = d["cpu_throttled_max_ratio"][burner]
assert 200 <= p95 <= 400, f"cpu-burner p95 {p95} not near its 300m limit"
assert throttle > 0.5, f"cpu-burner throttle {throttle} should be high"
# idle-hog requests 500m and uses ~0. Use the usage/request RATIO: absolute
# averages are diluted by how much of the window has data, ratios are not.
hog = rows["idle-hog"]
usage = d["cpu_usage_avg_millicores"][hog]
request = d["cpu_request_avg_millicores"][hog]
assert request > 0, "idle-hog has no recorded requests"
ratio = usage / request
assert ratio < 0.1, f"idle-hog uses {ratio:.0%} of its requests, expected ~0"
assert d["cpu_waste_millicores"][hog] > 0, "idle-hog waste should be positive"
print(f"workloads OK: burner p95={p95:.0f}mc throttle={throttle:.0%}, hog usage/request={ratio:.1%}")
EOF

echo "==> asserting tenancy (namespace label + pod label rules)"
curl -sf -X PUT localhost:18080/api/rules -H 'content-type: application/json' -d '[
  {"matcher_type":"namespace_label","match_key":"tenant","tenant_source":"label_value"},
  {"matcher_type":"pod_label","match_key":"team","tenant_source":"label_value"}
]' >/dev/null
TENANTS=$(curl -sf "localhost:18080/api/tenants?from=$FROM")
echo "$TENANTS" | python3 -c '
import json, sys
tenants = {t["tenant"] for t in json.load(sys.stdin)}
assert "acme" in tenants, f"tenant acme (loadgen ns label) missing: {tenants}"
print(f"tenants OK: {sorted(tenants)}")
'

if $HA; then
  echo "==> asserting replication (both replicas hold data)"
  for pod in $(kubectl -n "$NS" get pods -l clickhouse.com/cluster="$RELEASE" -o name | head -2); do
    COUNT=$(kubectl -n "$NS" exec "$pod" -c clickhouse -- clickhouse-client --query "SELECT count() FROM ekokube.pod_usage" 2>/dev/null || echo 0)
    echo "  $pod: $COUNT rows"
    [ "$COUNT" -gt 0 ] || fail "replica $pod has no data"
  done
fi

echo "E2E OK"
