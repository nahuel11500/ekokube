#!/usr/bin/env bash
# Installs ekokube as two Helm releases: the ClickHouse operator first (so its
# CRDs are established), then ekokube (agent, server, ClickHouse CRs, schema).
# Two releases is deliberate — Helm validates a release against the API server
# up front, so a ClickHouseCluster CR can't live in the same release as the
# CRDs that define it.
#
# Usage: deploy/install.sh [-n NAMESPACE] [--ha] [extra helm args for ekokube...]
set -euo pipefail
cd "$(dirname "$0")/.."

NS=ekokube
RELEASE=ekokube
HA=false
SKIP_OPERATOR=false
EXTRA=()
while [ $# -gt 0 ]; do
  case "$1" in
    -n|--namespace) NS="$2"; shift 2 ;;
    # Release name for the ekokube release. ekokube's cluster-scoped RBAC is
    # named after it, so distinct names are required to run more than one
    # ekokube in a cluster.
    -r|--release) RELEASE="$2"; shift 2 ;;
    --ha) HA=true; shift ;;
    # The operator is cluster-wide and independent of ekokube — skip installing
    # it if one already runs (it will reconcile the CRs this chart creates).
    --skip-operator) SKIP_OPERATOR=true; shift ;;
    *) EXTRA+=("$1"); shift ;;
  esac
done

OPERATOR_CHART="oci://ghcr.io/clickhouse/clickhouse-operator-helm"
OPERATOR_VERSION="0.0.6"

if $SKIP_OPERATOR; then
  echo "==> [1/2] skipping operator install (--skip-operator)"
  if ! kubectl get crd clickhouseclusters.clickhouse.com keeperclusters.clickhouse.com >/dev/null 2>&1; then
    echo "ERROR: ClickHouse operator CRDs not found. Install the operator first, or drop --skip-operator." >&2
    exit 1
  fi
else
  echo "==> [1/2] ClickHouse operator (release: clickhouse-operator, watching ns: $NS)"
  # watchNamespaces scopes the operator to ONLY reconcile CRs in this namespace,
  # so it coexists with any other ClickHouse operator elsewhere in the cluster.
  # The admission webhook only validates CRs (which we generate); disabling it
  # drops the cert-manager prerequisite. Re-enable + install cert-manager if you
  # apply hand-written ClickHouseCluster CRs you want validated.
  helm upgrade --install clickhouse-operator "$OPERATOR_CHART" --version "$OPERATOR_VERSION" \
    -n "$NS" --create-namespace \
    --set "controller.watchNamespaces={$NS}" \
    --set webhook.enabled=false \
    --set certManager.enabled=false \
    --set metrics.secure=false \
    --wait
fi

echo "==> waiting for ClickHouse CRDs to be established"
kubectl wait --for=condition=Established --timeout=120s \
  crd/clickhouseclusters.clickhouse.com crd/keeperclusters.clickhouse.com

echo "==> [2/2] ekokube (release: $RELEASE)"
helm upgrade --install "$RELEASE" deploy/helm/ekokube \
  -n "$NS" --create-namespace \
  --set clickhouse.ha="$HA" \
  "${EXTRA[@]}"

echo "==> done. Pods:"
kubectl -n "$NS" get pods
