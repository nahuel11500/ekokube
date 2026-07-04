//! Main collection loop: cgroups (usage) + kubelet (metadata) → ClickHouse.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;

use crate::cgroup;
use crate::config::Config;
use crate::kubelet::{KubeletClient, PodMeta};
use crate::metrics::Metrics;
use crate::model::{NodeUsageRow, PodMetaRow, PodUsageRow};
use crate::node;
use crate::sink::Sink;

/// Previous cumulative counters for one pod, used to compute rates.
struct PodPrev {
    cpu_usage_usec: u64,
    cpu_nr_periods: u64,
    cpu_nr_throttled: u64,
    at: Instant,
}

pub struct Collector {
    config: Config,
    kubelet: KubeletClient,
    sink: Sink,
    metrics: Arc<Metrics>,
    cgroup_root: PathBuf,
    pod_prev: HashMap<String, PodPrev>,
    node_cpu_prev: Option<(node::CpuTimes, Instant)>,
    meta_cache: HashMap<String, PodMeta>,
    meta_fetched_at: Option<Instant>,
    /// Hash of the last pod_meta row written per pod, to only upsert on change.
    meta_written: HashMap<String, u64>,
}

fn now_epoch() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0)
}

fn meta_hash(meta: &PodMeta) -> u64 {
    let mut hasher = DefaultHasher::new();
    meta.namespace.hash(&mut hasher);
    meta.name.hash(&mut hasher);
    meta.labels.hash(&mut hasher);
    meta.workload_kind.hash(&mut hasher);
    meta.workload_name.hash(&mut hasher);
    meta.cpu_request_millis.hash(&mut hasher);
    meta.cpu_limit_millis.hash(&mut hasher);
    meta.mem_request_bytes.hash(&mut hasher);
    meta.mem_limit_bytes.hash(&mut hasher);
    hasher.finish()
}

impl Collector {
    pub fn new(config: Config, metrics: Arc<Metrics>) -> Result<Self> {
        let kubelet = KubeletClient::new(
            &config.kubelet_url,
            &config.kubelet_token_path,
            config.kubelet_insecure_tls,
        )?;
        let sink = Sink::new(&config, metrics.clone());
        let cgroup_root = PathBuf::from(&config.cgroup_root);
        Ok(Self {
            config,
            kubelet,
            sink,
            metrics,
            cgroup_root,
            pod_prev: HashMap::new(),
            node_cpu_prev: None,
            meta_cache: HashMap::new(),
            meta_fetched_at: None,
            meta_written: HashMap::new(),
        })
    }

    pub async fn run(mut self) -> Result<()> {
        let mut ticker = tokio::time::interval(Duration::from_secs(self.config.interval_secs));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let started = Instant::now();
                    if let Err(e) = self.collect_once().await {
                        tracing::error!(error = %e, "collection cycle failed");
                    }
                    let elapsed_ms = started.elapsed().as_millis() as u64;
                    self.metrics.cycles_total.fetch_add(1, Ordering::Relaxed);
                    self.metrics.cycle_duration_ms.store(elapsed_ms, Ordering::Relaxed);
                    tracing::debug!(elapsed_ms, "cycle done");
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("shutting down");
                    return Ok(());
                }
            }
        }
    }

    async fn refresh_meta_if_needed(&mut self, unknown_uids: bool) {
        let stale = self
            .meta_fetched_at
            .map(|t| t.elapsed().as_secs() >= self.config.meta_interval_secs)
            .unwrap_or(true);
        if !stale && !unknown_uids {
            return;
        }
        match self.kubelet.fetch_pods().await {
            Ok(pods) => {
                self.meta_cache = pods;
                self.meta_fetched_at = Some(Instant::now());
            }
            Err(e) => {
                tracing::warn!(error = %e, "kubelet /pods fetch failed, using cached metadata")
            }
        }
    }

    async fn collect_once(&mut self) -> Result<()> {
        let ts = now_epoch();
        let pods = cgroup::discover_pods(&self.cgroup_root);

        let unknown = pods.iter().any(|p| !self.meta_cache.contains_key(&p.uid));
        self.refresh_meta_if_needed(unknown).await;

        let mut pod_rows = Vec::with_capacity(pods.len());
        let mut meta_rows = Vec::new();
        let mut seen: HashSet<String> = HashSet::with_capacity(pods.len());

        for pod in &pods {
            seen.insert(pod.uid.clone());
            let stats = cgroup::read_pod_stats(&pod.path);
            let now = Instant::now();

            // Rates from cumulative counters; first sighting yields no rate yet.
            let (cpu_millicores, throttled_ratio, sample_secs) = match self.pod_prev.get(&pod.uid) {
                Some(prev) => {
                    let wall_usec = now.duration_since(prev.at).as_micros() as u64;
                    let cpu = if wall_usec > 0 {
                        stats.cpu_usage_usec.saturating_sub(prev.cpu_usage_usec) as f32
                            / wall_usec as f32
                            * 1000.0
                    } else {
                        0.0
                    };
                    let periods = stats.cpu_nr_periods.saturating_sub(prev.cpu_nr_periods);
                    let throttled = stats.cpu_nr_throttled.saturating_sub(prev.cpu_nr_throttled);
                    let ratio = if periods > 0 {
                        throttled as f32 / periods as f32
                    } else {
                        0.0
                    };
                    (Some(cpu), ratio, wall_usec as f32 / 1_000_000.0)
                }
                None => (None, 0.0, 0.0),
            };
            self.pod_prev.insert(
                pod.uid.clone(),
                PodPrev {
                    cpu_usage_usec: stats.cpu_usage_usec,
                    cpu_nr_periods: stats.cpu_nr_periods,
                    cpu_nr_throttled: stats.cpu_nr_throttled,
                    at: now,
                },
            );
            let Some(cpu_millicores) = cpu_millicores else {
                continue; // need two samples for a rate
            };

            let Some(meta) = self.meta_cache.get(&pod.uid) else {
                // Pod cgroup exists but kubelet doesn't report it (yet, or anymore).
                continue;
            };

            pod_rows.push(PodUsageRow {
                ts,
                sample_secs,
                node: self.config.node_name.clone(),
                pod_uid: pod.uid.clone(),
                namespace: meta.namespace.clone(),
                pod_name: meta.name.clone(),
                workload_kind: meta.workload_kind.clone(),
                workload_name: meta.workload_name.clone(),
                qos: if meta.qos_class.is_empty() {
                    pod.qos.as_str().to_string()
                } else {
                    meta.qos_class.clone()
                },
                cpu_usage_millicores: cpu_millicores,
                cpu_request_millicores: meta.cpu_request_millis as u32,
                cpu_limit_millicores: meta.cpu_limit_millis as u32,
                cpu_throttled_ratio: throttled_ratio,
                mem_working_set_bytes: stats.mem_working_set,
                mem_current_bytes: stats.mem_current,
                mem_request_bytes: meta.mem_request_bytes,
                mem_limit_bytes: meta.mem_limit_bytes,
                psi_cpu_some_ratio: stats.psi_cpu_some,
                psi_mem_some_ratio: stats.psi_mem_some,
            });

            let hash = meta_hash(meta);
            if self.meta_written.get(&pod.uid) != Some(&hash) {
                meta_rows.push(PodMetaRow {
                    pod_uid: pod.uid.clone(),
                    namespace: meta.namespace.clone(),
                    pod_name: meta.name.clone(),
                    node: self.config.node_name.clone(),
                    labels: meta.labels.clone(),
                    workload_kind: meta.workload_kind.clone(),
                    workload_name: meta.workload_name.clone(),
                    updated_at: ts,
                });
                self.meta_written.insert(pod.uid.clone(), hash);
            }
        }

        // Drop state for pods that no longer exist.
        self.pod_prev.retain(|uid, _| seen.contains(uid));
        self.meta_written.retain(|uid, _| seen.contains(uid));

        let node_rows = self.node_row(ts, pods.len() as u32);
        self.metrics
            .pods_scraped
            .store(pod_rows.len() as u64, Ordering::Relaxed);
        tracing::info!(
            pods = pod_rows.len(),
            meta_updates = meta_rows.len(),
            "cycle collected"
        );
        self.sink.write_cycle(pod_rows, node_rows, meta_rows).await;
        Ok(())
    }

    fn node_row(&mut self, ts: u32, pod_count: u32) -> Vec<NodeUsageRow> {
        let cpu = node::current_cpu_times();
        let mem = node::current_meminfo();
        let now = Instant::now();
        let row = match self.node_cpu_prev.replace((cpu, now)) {
            Some((prev, prev_at)) if cpu.total_ticks > prev.total_ticks => {
                let delta_total = (cpu.total_ticks - prev.total_ticks) as f32;
                let delta_used = cpu.used_ticks.saturating_sub(prev.used_ticks) as f32;
                Some(NodeUsageRow {
                    ts,
                    sample_secs: now.duration_since(prev_at).as_secs_f32(),
                    node: self.config.node_name.clone(),
                    cpu_used_millicores: delta_used / delta_total * cpu.num_cpus as f32 * 1000.0,
                    cpu_capacity_millicores: cpu.num_cpus * 1000,
                    mem_used_bytes: mem.used_bytes,
                    mem_total_bytes: mem.total_bytes,
                    pod_count,
                })
            }
            _ => None, // first cycle: no delta yet
        };
        row.into_iter().collect()
    }
}
