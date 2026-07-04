//! Batched ClickHouse writer with a bounded outage buffer.
//!
//! Each cycle's rows are appended to a per-table pending queue, then the
//! queues are drained front-first. A failed insert leaves the batch queued
//! for the next cycle, so a ClickHouse outage of a few minutes loses nothing.
//! The buffer is capped by total rows across tables; beyond the cap the
//! oldest batches are dropped (bounded memory) and counted in the metrics.

use std::collections::VecDeque;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use anyhow::Result;
use clickhouse::{Client, Row};
use serde::Serialize;

use crate::config::Config;
use crate::metrics::Metrics;
use crate::model::{NodeUsageRow, PodMetaRow, PodUsageRow};

struct Pending<R> {
    table: &'static str,
    batches: VecDeque<Vec<R>>,
    rows: usize,
}

impl<R> Pending<R> {
    fn new(table: &'static str) -> Self {
        Self {
            table,
            batches: VecDeque::new(),
            rows: 0,
        }
    }

    fn push(&mut self, batch: Vec<R>) {
        if batch.is_empty() {
            return;
        }
        self.rows += batch.len();
        self.batches.push_back(batch);
    }

    /// Drops oldest batches until at most `cap` rows remain. Returns rows dropped.
    fn enforce_cap(&mut self, cap: usize) -> usize {
        let mut dropped = 0;
        while self.rows > cap {
            let Some(oldest) = self.batches.pop_front() else {
                break;
            };
            self.rows -= oldest.len();
            dropped += oldest.len();
        }
        dropped
    }
}

pub struct Sink {
    client: Client,
    metrics: Arc<Metrics>,
    max_buffered_rows: usize,
    pod_usage: Pending<PodUsageRow>,
    node_usage: Pending<NodeUsageRow>,
    pod_meta: Pending<PodMetaRow>,
}

async fn insert_rows<R: Row + Serialize>(client: &Client, table: &str, rows: &[R]) -> Result<()> {
    let mut insert = client.insert(table)?;
    for row in rows {
        insert.write(row).await?;
    }
    insert.end().await?;
    Ok(())
}

/// Drains one queue front-first; stops at the first failing insert.
async fn flush<R: Row + Serialize>(client: &Client, pending: &mut Pending<R>, metrics: &Metrics) {
    while let Some(batch) = pending.batches.front() {
        match insert_rows(client, pending.table, batch).await {
            Ok(()) => {
                metrics
                    .rows_counter(pending.table)
                    .fetch_add(batch.len() as u64, Ordering::Relaxed);
                pending.rows -= batch.len();
                pending.batches.pop_front();
            }
            Err(e) => {
                metrics
                    .insert_failures_total
                    .fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    table = pending.table,
                    queued_rows = pending.rows,
                    error = %e,
                    "insert failed; batch kept for next cycle"
                );
                break;
            }
        }
    }
}

impl Sink {
    pub fn new(config: &Config, metrics: Arc<Metrics>) -> Self {
        let client = Client::default()
            .with_url(&config.clickhouse_url)
            .with_database(&config.clickhouse_database)
            .with_user(&config.clickhouse_user)
            .with_password(&config.clickhouse_password)
            .with_option("async_insert", "1")
            .with_option("wait_for_async_insert", "0");
        Self {
            client,
            metrics,
            max_buffered_rows: config.buffer_max_rows,
            pod_usage: Pending::new("pod_usage"),
            node_usage: Pending::new("node_usage"),
            pod_meta: Pending::new("pod_meta"),
        }
    }

    pub async fn write_cycle(
        &mut self,
        pod_usage: Vec<PodUsageRow>,
        node_usage: Vec<NodeUsageRow>,
        pod_meta: Vec<PodMetaRow>,
    ) {
        self.pod_usage.push(pod_usage);
        self.node_usage.push(node_usage);
        self.pod_meta.push(pod_meta);

        // Never drop pod_meta (tiny, and labels are needed for tenancy);
        // usage tables share the cap proportionally to their volume.
        let mut dropped = self.pod_usage.enforce_cap(self.max_buffered_rows * 9 / 10);
        dropped += self.node_usage.enforce_cap(self.max_buffered_rows / 10);
        if dropped > 0 {
            self.metrics
                .buffer_dropped_rows_total
                .fetch_add(dropped as u64, Ordering::Relaxed);
            tracing::error!(dropped, "outage buffer full; dropped oldest rows");
        }

        flush(&self.client, &mut self.pod_usage, &self.metrics).await;
        flush(&self.client, &mut self.node_usage, &self.metrics).await;
        flush(&self.client, &mut self.pod_meta, &self.metrics).await;

        let buffered = self.pod_usage.rows + self.node_usage.rows + self.pod_meta.rows;
        self.metrics
            .buffer_rows
            .store(buffered as u64, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn batch(n: usize) -> Vec<u32> {
        vec![0; n]
    }

    #[test]
    fn push_tracks_rows_and_skips_empty() {
        let mut p: Pending<u32> = Pending::new("t");
        p.push(batch(0));
        p.push(batch(3));
        p.push(batch(2));
        assert_eq!(p.rows, 5);
        assert_eq!(p.batches.len(), 2);
    }

    #[test]
    fn cap_drops_oldest_first() {
        let mut p: Pending<u32> = Pending::new("t");
        p.push(vec![1, 1, 1]);
        p.push(vec![2, 2]);
        p.push(vec![3]);
        let dropped = p.enforce_cap(4);
        assert_eq!(dropped, 3); // the oldest batch of 3
        assert_eq!(p.rows, 3);
        assert_eq!(p.batches.front().unwrap()[0], 2);

        // dropping continues until under the cap
        let dropped = p.enforce_cap(0);
        assert_eq!(dropped, 3);
        assert_eq!(p.rows, 0);
        assert!(p.batches.is_empty());
    }

    #[test]
    fn cap_noop_when_under() {
        let mut p: Pending<u32> = Pending::new("t");
        p.push(batch(5));
        assert_eq!(p.enforce_cap(5), 0);
        assert_eq!(p.rows, 5);
    }
}
