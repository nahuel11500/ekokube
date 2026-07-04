//! Pod metadata from the node-local kubelet /pods endpoint.
//!
//! This never touches the API server: the agent talks to the kubelet on its own
//! node, and the kubelet caches the ServiceAccount auth check.

use std::collections::{BTreeMap, HashMap};

use anyhow::{Context, Result};
use serde::Deserialize;

use ekokube_common::quantity;

#[derive(Debug, Clone)]
pub struct PodMeta {
    pub uid: String,
    pub name: String,
    pub namespace: String,
    pub labels: Vec<(String, String)>,
    pub workload_kind: String,
    pub workload_name: String,
    pub qos_class: String,
    pub cpu_request_millis: u64,
    pub cpu_limit_millis: u64,
    pub mem_request_bytes: u64,
    pub mem_limit_bytes: u64,
}

#[derive(Deserialize)]
struct PodList {
    items: Vec<Pod>,
}

#[derive(Deserialize)]
struct Pod {
    metadata: Metadata,
    spec: Spec,
    #[serde(default)]
    status: Status,
}

#[derive(Deserialize)]
struct Metadata {
    name: String,
    namespace: String,
    uid: String,
    #[serde(default)]
    labels: BTreeMap<String, String>,
    #[serde(default, rename = "ownerReferences")]
    owner_references: Vec<OwnerReference>,
}

#[derive(Deserialize)]
struct OwnerReference {
    kind: String,
    name: String,
    #[serde(default)]
    controller: bool,
}

#[derive(Deserialize)]
struct Spec {
    #[serde(default)]
    containers: Vec<Container>,
}

#[derive(Deserialize)]
struct Container {
    #[serde(default)]
    resources: Resources,
}

#[derive(Deserialize, Default)]
struct Resources {
    #[serde(default)]
    requests: BTreeMap<String, String>,
    #[serde(default)]
    limits: BTreeMap<String, String>,
}

#[derive(Deserialize, Default)]
struct Status {
    #[serde(rename = "qosClass")]
    qos_class: Option<String>,
}

/// Resolves the controlling workload from owner references.
/// ReplicaSets are mapped back to their Deployment by stripping the
/// pod-template-hash suffix from the ReplicaSet name.
pub fn resolve_workload(owner_refs: &[(String, String, bool)], pod_name: &str) -> (String, String) {
    let controller = owner_refs
        .iter()
        .find(|(_, _, is_controller)| *is_controller)
        .or_else(|| owner_refs.first());
    match controller {
        Some((kind, name, _)) if kind == "ReplicaSet" => {
            let deployment = match name.rsplit_once('-') {
                Some((prefix, _hash)) if !prefix.is_empty() => prefix.to_string(),
                _ => name.clone(),
            };
            ("Deployment".to_string(), deployment)
        }
        Some((kind, name, _)) => (kind.clone(), name.clone()),
        None => ("Pod".to_string(), pod_name.to_string()),
    }
}

fn sum_quantities<F: Fn(&str) -> Option<u64>>(
    containers: &[Container],
    section: fn(&Resources) -> &BTreeMap<String, String>,
    key: &str,
    parse: F,
) -> u64 {
    containers
        .iter()
        .filter_map(|c| section(&c.resources).get(key))
        .filter_map(|q| parse(q))
        .sum()
}

fn to_meta(pod: Pod) -> PodMeta {
    let owner_refs: Vec<(String, String, bool)> = pod
        .metadata
        .owner_references
        .iter()
        .map(|o| (o.kind.clone(), o.name.clone(), o.controller))
        .collect();
    let (workload_kind, workload_name) = resolve_workload(&owner_refs, &pod.metadata.name);
    let containers = &pod.spec.containers;
    PodMeta {
        uid: pod.metadata.uid,
        name: pod.metadata.name,
        namespace: pod.metadata.namespace,
        labels: pod.metadata.labels.into_iter().collect(),
        workload_kind,
        workload_name,
        qos_class: pod.status.qos_class.unwrap_or_default(),
        cpu_request_millis: sum_quantities(
            containers,
            |r| &r.requests,
            "cpu",
            quantity::parse_cpu_millis,
        ),
        cpu_limit_millis: sum_quantities(
            containers,
            |r| &r.limits,
            "cpu",
            quantity::parse_cpu_millis,
        ),
        mem_request_bytes: sum_quantities(
            containers,
            |r| &r.requests,
            "memory",
            quantity::parse_mem_bytes,
        ),
        mem_limit_bytes: sum_quantities(
            containers,
            |r| &r.limits,
            "memory",
            quantity::parse_mem_bytes,
        ),
    }
}

pub struct KubeletClient {
    http: reqwest::Client,
    pods_url: String,
    token_path: String,
}

impl KubeletClient {
    pub fn new(kubelet_url: &str, token_path: &str, insecure_tls: bool) -> Result<Self> {
        let http = reqwest::Client::builder()
            .danger_accept_invalid_certs(insecure_tls)
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .context("building kubelet http client")?;
        Ok(Self {
            http,
            pods_url: format!("{}/pods", kubelet_url.trim_end_matches('/')),
            token_path: token_path.to_string(),
        })
    }

    /// Fetches all pods on this node, keyed by pod UID.
    pub async fn fetch_pods(&self) -> Result<HashMap<String, PodMeta>> {
        let mut request = self.http.get(&self.pods_url);
        if let Ok(token) = tokio::fs::read_to_string(&self.token_path).await {
            request = request.bearer_auth(token.trim());
        }
        let response = request.send().await.context("kubelet /pods request")?;
        let response = response
            .error_for_status()
            .context("kubelet /pods status")?;
        let pod_list: PodList = response.json().await.context("decoding kubelet /pods")?;
        Ok(pod_list
            .items
            .into_iter()
            .map(|p| {
                let meta = to_meta(p);
                (meta.uid.clone(), meta)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replicaset_maps_to_deployment() {
        let refs = vec![(
            "ReplicaSet".to_string(),
            "api-server-7d9c8b6f5".to_string(),
            true,
        )];
        assert_eq!(
            resolve_workload(&refs, "api-server-7d9c8b6f5-x2kl9"),
            ("Deployment".to_string(), "api-server".to_string())
        );
    }

    #[test]
    fn other_kinds_pass_through() {
        let refs = vec![("StatefulSet".to_string(), "clickhouse".to_string(), true)];
        assert_eq!(
            resolve_workload(&refs, "clickhouse-0"),
            ("StatefulSet".to_string(), "clickhouse".to_string())
        );
        let refs = vec![("DaemonSet".to_string(), "ekokube-agent".to_string(), true)];
        assert_eq!(resolve_workload(&refs, "x").0, "DaemonSet");
    }

    #[test]
    fn bare_pod_is_its_own_workload() {
        assert_eq!(
            resolve_workload(&[], "debug-pod"),
            ("Pod".to_string(), "debug-pod".to_string())
        );
    }

    #[test]
    fn parses_kubelet_pod_json() {
        let json = r#"{
          "items": [{
            "metadata": {
              "name": "web-6f8d4b9c7d-abcde",
              "namespace": "shop",
              "uid": "11111111-2222-3333-4444-555555555555",
              "labels": {"app": "web", "team": "checkout"},
              "ownerReferences": [{"kind": "ReplicaSet", "name": "web-6f8d4b9c7d", "controller": true}]
            },
            "spec": {
              "containers": [
                {"resources": {"requests": {"cpu": "250m", "memory": "256Mi"}, "limits": {"cpu": "1", "memory": "512Mi"}}},
                {"resources": {"requests": {"cpu": "50m", "memory": "64Mi"}}}
              ]
            },
            "status": {"qosClass": "Burstable"}
          }]
        }"#;
        let list: PodList = serde_json::from_str(json).unwrap();
        let meta = to_meta(list.items.into_iter().next().unwrap());
        assert_eq!(meta.namespace, "shop");
        assert_eq!(meta.workload_kind, "Deployment");
        assert_eq!(meta.workload_name, "web");
        assert_eq!(meta.cpu_request_millis, 300);
        assert_eq!(meta.cpu_limit_millis, 1000);
        assert_eq!(meta.mem_request_bytes, (256 + 64) * 1024 * 1024);
        assert_eq!(meta.mem_limit_bytes, 512 * 1024 * 1024);
        assert_eq!(meta.qos_class, "Burstable");
        assert_eq!(meta.labels.len(), 2);
    }
}
