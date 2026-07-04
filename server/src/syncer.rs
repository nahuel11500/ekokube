//! Syncs namespace and node metadata (labels, allocatable) from the API server
//! into ClickHouse dimension tables. This is the only component that talks to
//! the API server, and it only lists two small collections periodically.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use clickhouse::{Client, Row};
use k8s_openapi::api::core::v1::{Namespace, Node};
use kube::api::{Api, ListParams};
use serde::Serialize;

use ekokube_common::quantity;

#[derive(Row, Serialize)]
struct NamespaceRow {
    name: String,
    labels: Vec<(String, String)>,
    updated_at: u32,
}

#[derive(Row, Serialize)]
struct NodeRow {
    name: String,
    labels: Vec<(String, String)>,
    cpu_allocatable_millicores: u32,
    mem_allocatable_bytes: u64,
    updated_at: u32,
}

fn now_epoch() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

fn labels_of(meta: &kube::api::ObjectMeta) -> Vec<(String, String)> {
    meta.labels
        .clone()
        .unwrap_or_default()
        .into_iter()
        .collect()
}

pub async fn run(ch: Client, interval_secs: u64) -> Result<()> {
    let kube_client = kube::Client::try_default().await?;
    let namespaces: Api<Namespace> = Api::all(kube_client.clone());
    let nodes: Api<Node> = Api::all(kube_client);
    let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        if let Err(e) = sync_once(&ch, &namespaces, &nodes).await {
            tracing::warn!(error = %e, "sync cycle failed");
        }
    }
}

async fn sync_once(ch: &Client, namespaces: &Api<Namespace>, nodes: &Api<Node>) -> Result<()> {
    let ts = now_epoch();

    let ns_list = namespaces.list(&ListParams::default()).await?;
    let mut insert = ch.insert("namespaces")?;
    for ns in &ns_list.items {
        insert
            .write(&NamespaceRow {
                name: ns.metadata.name.clone().unwrap_or_default(),
                labels: labels_of(&ns.metadata),
                updated_at: ts,
            })
            .await?;
    }
    insert.end().await?;

    let node_list = nodes.list(&ListParams::default()).await?;
    let mut insert = ch.insert("nodes")?;
    for node in &node_list.items {
        let allocatable = node.status.as_ref().and_then(|s| s.allocatable.as_ref());
        let cpu = allocatable
            .and_then(|a| a.get("cpu"))
            .and_then(|q| quantity::parse_cpu_millis(&q.0))
            .unwrap_or(0) as u32;
        let mem = allocatable
            .and_then(|a| a.get("memory"))
            .and_then(|q| quantity::parse_mem_bytes(&q.0))
            .unwrap_or(0);
        insert
            .write(&NodeRow {
                name: node.metadata.name.clone().unwrap_or_default(),
                labels: labels_of(&node.metadata),
                cpu_allocatable_millicores: cpu,
                mem_allocatable_bytes: mem,
                updated_at: ts,
            })
            .await?;
    }
    insert.end().await?;

    tracing::debug!(
        namespaces = ns_list.items.len(),
        nodes = node_list.items.len(),
        "synced"
    );
    Ok(())
}
