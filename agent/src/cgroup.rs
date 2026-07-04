//! Reads pod resource usage directly from the cgroup v2 filesystem.
//!
//! Supports both cgroup drivers:
//! - systemd:  kubepods.slice/kubepods-burstable.slice/kubepods-burstable-pod<uid_>.slice
//!   (pod UID dashes are encoded as underscores in slice names)
//! - cgroupfs: kubepods/burstable/pod<uid>

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QosClass {
    Guaranteed,
    Burstable,
    BestEffort,
}

impl QosClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            QosClass::Guaranteed => "Guaranteed",
            QosClass::Burstable => "Burstable",
            QosClass::BestEffort => "BestEffort",
        }
    }
}

#[derive(Debug)]
pub struct PodCgroup {
    /// Canonical pod UID (dashes, as seen by the Kubernetes API).
    pub uid: String,
    pub path: PathBuf,
    pub qos: QosClass,
}

/// Raw counters/gauges read from one pod cgroup.
#[derive(Debug, Default, Clone)]
pub struct PodStats {
    /// Cumulative CPU time consumed, microseconds (cpu.stat usage_usec).
    pub cpu_usage_usec: u64,
    /// Cumulative number of enforcement periods (cpu.stat nr_periods).
    pub cpu_nr_periods: u64,
    /// Cumulative number of throttled periods (cpu.stat nr_throttled).
    pub cpu_nr_throttled: u64,
    /// Total memory currently used, bytes (memory.current).
    pub mem_current: u64,
    /// Working set: memory.current minus inactive file cache, bytes.
    pub mem_working_set: u64,
    /// PSI "some" avg10 for CPU, as a ratio in [0,1].
    pub psi_cpu_some: f32,
    /// PSI "some" avg10 for memory, as a ratio in [0,1].
    pub psi_mem_some: f32,
}

/// Extracts a canonical pod UID from a cgroup directory name, if it is a pod
/// slice. Slice prefixes vary with the kubelet's cgroup config (`kubepods-`,
/// `kubelet-kubepods-`, bare `pod<uid>` for the cgroupfs driver), so this
/// only requires the name to end in `pod<uid>`; QoS is read from the name
/// when present, otherwise from the parent directory.
pub fn parse_pod_dir_name(name: &str) -> Option<(String, Option<QosClass>)> {
    let stem = name.strip_suffix(".slice").unwrap_or(name);
    // rfind so the "pod" inside "kubepods" never matches: a valid stem always
    // has the UID-introducing "pod" last.
    let pod_at = stem.rfind("pod")?;
    let uid = stem[pod_at + 3..].replace('_', "-");
    if !is_pod_uid(&uid) {
        return None;
    }
    let prefix = &stem[..pod_at];
    let qos = if prefix.contains("burstable") {
        Some(QosClass::Burstable)
    } else if prefix.contains("besteffort") {
        Some(QosClass::BestEffort)
    } else if prefix.contains("kubepods") {
        Some(QosClass::Guaranteed)
    } else {
        None // bare "pod<uid>" (cgroupfs): QoS comes from the parent dir
    };
    Some((uid, qos))
}

fn is_pod_uid(s: &str) -> bool {
    s.len() == 36
        && s.bytes().enumerate().all(|(i, b)| match i {
            8 | 13 | 18 | 23 => b == b'-',
            _ => b.is_ascii_hexdigit(),
        })
}

/// Locates the kubepods root: a directory whose name contains "kubepods",
/// searched breadth-first a few levels deep. The location varies with the
/// kubelet's cgroup config — `kubepods.slice` on plain systemd nodes,
/// `kubelet.slice/kubelet-kubepods.slice` when kubeletCgroups is set (kind),
/// `kubepods` with the cgroupfs driver.
pub fn find_kubepods_root(cgroup_root: &Path) -> Option<PathBuf> {
    let mut level = vec![cgroup_root.to_path_buf()];
    for _ in 0..3 {
        let mut next = Vec::new();
        for dir in &level {
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.contains("kubepods") {
                    return Some(entry.path());
                }
                // .scope dirs (containers, units) never contain kubepods
                if !name.ends_with(".scope") {
                    next.push(entry.path());
                }
            }
        }
        level = next;
    }
    None
}

/// Discovers all pod cgroups under the cgroup v2 root.
pub fn discover_pods(cgroup_root: &Path) -> Vec<PodCgroup> {
    let mut pods = Vec::with_capacity(256);
    let Some(kubepods) = find_kubepods_root(cgroup_root) else {
        return pods;
    };
    scan_level(&kubepods, None, 0, &mut pods);
    pods
}

fn scan_level(dir: &Path, parent_qos: Option<QosClass>, depth: u32, out: &mut Vec<PodCgroup>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if let Some((uid, qos)) = parse_pod_dir_name(&name) {
            out.push(PodCgroup {
                uid,
                path: entry.path(),
                qos: qos.or(parent_qos).unwrap_or(QosClass::Guaranteed),
            });
        } else if parent_qos.is_none() && depth == 0 {
            // QoS sub-level: *burstable*.slice / burstable / besteffort
            let qos = if name.contains("burstable") {
                Some(QosClass::Burstable)
            } else if name.contains("besteffort") {
                Some(QosClass::BestEffort)
            } else {
                None
            };
            if qos.is_some() {
                scan_level(&entry.path(), qos, depth + 1, out);
            }
        }
    }
}

/// Reads usage stats for one pod cgroup. Missing files (e.g. PSI disabled) yield zeros.
pub fn read_pod_stats(pod_path: &Path) -> PodStats {
    let mut stats = PodStats::default();

    if let Ok(content) = fs::read_to_string(pod_path.join("cpu.stat")) {
        for line in content.lines() {
            let mut parts = line.split_ascii_whitespace();
            match (parts.next(), parts.next()) {
                (Some("usage_usec"), Some(v)) => stats.cpu_usage_usec = v.parse().unwrap_or(0),
                (Some("nr_periods"), Some(v)) => stats.cpu_nr_periods = v.parse().unwrap_or(0),
                (Some("nr_throttled"), Some(v)) => stats.cpu_nr_throttled = v.parse().unwrap_or(0),
                _ => {}
            }
        }
    }

    if let Ok(content) = fs::read_to_string(pod_path.join("memory.current")) {
        stats.mem_current = content.trim().parse().unwrap_or(0);
    }

    let mut inactive_file = 0u64;
    if let Ok(content) = fs::read_to_string(pod_path.join("memory.stat")) {
        for line in content.lines() {
            if let Some(v) = line.strip_prefix("inactive_file ") {
                inactive_file = v.trim().parse().unwrap_or(0);
                break;
            }
        }
    }
    stats.mem_working_set = stats.mem_current.saturating_sub(inactive_file);

    stats.psi_cpu_some = read_psi_some_avg10(&pod_path.join("cpu.pressure"));
    stats.psi_mem_some = read_psi_some_avg10(&pod_path.join("memory.pressure"));
    stats
}

/// Parses the "some avg10=X.XX ..." line of a PSI file into a [0,1] ratio.
fn read_psi_some_avg10(path: &Path) -> f32 {
    let Ok(content) = fs::read_to_string(path) else {
        return 0.0;
    };
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("some ") {
            for field in rest.split_ascii_whitespace() {
                if let Some(v) = field.strip_prefix("avg10=") {
                    return v.parse::<f32>().unwrap_or(0.0) / 100.0;
                }
            }
        }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parses_systemd_slice_names() {
        let (uid, qos) =
            parse_pod_dir_name("kubepods-burstable-pod3b4c1f9e_1a2b_4c3d_9e8f_001122334455.slice")
                .unwrap();
        assert_eq!(uid, "3b4c1f9e-1a2b-4c3d-9e8f-001122334455");
        assert_eq!(qos, Some(QosClass::Burstable));

        let (uid, qos) =
            parse_pod_dir_name("kubepods-pod00000000_0000_0000_0000_000000000000.slice").unwrap();
        assert_eq!(uid, "00000000-0000-0000-0000-000000000000");
        assert_eq!(qos, Some(QosClass::Guaranteed));

        let (_, qos) =
            parse_pod_dir_name("kubepods-besteffort-podaaaaaaaa_bbbb_cccc_dddd_eeeeeeeeeeee.slice")
                .unwrap();
        assert_eq!(qos, Some(QosClass::BestEffort));
    }

    #[test]
    fn parses_cgroupfs_names() {
        let (uid, qos) = parse_pod_dir_name("pod3b4c1f9e-1a2b-4c3d-9e8f-001122334455").unwrap();
        assert_eq!(uid, "3b4c1f9e-1a2b-4c3d-9e8f-001122334455");
        assert_eq!(qos, None);
    }

    #[test]
    fn parses_kind_style_prefixed_slices() {
        // kubelet with kubeletCgroups set (e.g. kind) prefixes the slice names
        let (uid, qos) = parse_pod_dir_name(
            "kubelet-kubepods-burstable-pod48643658_2204_40c0_aa69_2bc5eec55d38.slice",
        )
        .unwrap();
        assert_eq!(uid, "48643658-2204-40c0-aa69-2bc5eec55d38");
        assert_eq!(qos, Some(QosClass::Burstable));

        let (_, qos) =
            parse_pod_dir_name("kubelet-kubepods-pod48643658_2204_40c0_aa69_2bc5eec55d38.slice")
                .unwrap();
        assert_eq!(qos, Some(QosClass::Guaranteed));
    }

    #[test]
    fn finds_nested_kubepods_root() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let kubepods = root.join("kubelet.slice").join("kubelet-kubepods.slice");
        fs::create_dir_all(&kubepods).unwrap();
        fs::create_dir_all(root.join("system.slice").join("ssh.service")).unwrap();
        assert_eq!(find_kubepods_root(root).unwrap(), kubepods);
    }

    #[test]
    fn rejects_non_pod_dirs() {
        assert!(parse_pod_dir_name("kubepods-burstable.slice").is_none());
        assert!(parse_pod_dir_name("system.slice").is_none());
        assert!(parse_pod_dir_name("podinfo").is_none());
        assert!(parse_pod_dir_name("kubepods-podnotauid.slice").is_none());
    }

    fn write(path: &Path, content: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    #[test]
    fn discovers_and_reads_fake_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let pod = root
            .join("kubepods.slice")
            .join("kubepods-burstable.slice")
            .join("kubepods-burstable-pod11111111_2222_3333_4444_555555555555.slice");
        write(
            &pod.join("cpu.stat"),
            "usage_usec 5000000\nuser_usec 4000000\nsystem_usec 1000000\nnr_periods 100\nnr_throttled 25\nthrottled_usec 900000\n",
        );
        write(&pod.join("memory.current"), "104857600\n");
        write(
            &pod.join("memory.stat"),
            "anon 73400320\nfile 31457280\ninactive_file 20971520\nactive_file 10485760\n",
        );
        write(
            &pod.join("cpu.pressure"),
            "some avg10=1.50 avg60=0.80 avg300=0.20 total=123456\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=0\n",
        );

        let pods = discover_pods(root);
        assert_eq!(pods.len(), 1);
        assert_eq!(pods[0].uid, "11111111-2222-3333-4444-555555555555");
        assert_eq!(pods[0].qos, QosClass::Burstable);

        let stats = read_pod_stats(&pods[0].path);
        assert_eq!(stats.cpu_usage_usec, 5_000_000);
        assert_eq!(stats.cpu_nr_periods, 100);
        assert_eq!(stats.cpu_nr_throttled, 25);
        assert_eq!(stats.mem_current, 104_857_600);
        assert_eq!(stats.mem_working_set, 104_857_600 - 20_971_520);
        assert!((stats.psi_cpu_some - 0.015).abs() < 1e-6);
        assert_eq!(stats.psi_mem_some, 0.0);
    }
}
