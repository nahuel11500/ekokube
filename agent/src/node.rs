//! Node-level CPU/memory from /proc (host-global values, visible from any container).

use std::fs;

#[derive(Debug, Clone, Copy, Default)]
pub struct CpuTimes {
    pub used_ticks: u64,
    pub total_ticks: u64,
    pub num_cpus: u32,
}

/// Parses /proc/stat: aggregate cpu line + counts per-cpu lines.
pub fn read_cpu_times(proc_stat: &str) -> CpuTimes {
    let mut times = CpuTimes::default();
    for line in proc_stat.lines() {
        if let Some(rest) = line.strip_prefix("cpu ") {
            let fields: Vec<u64> = rest
                .split_ascii_whitespace()
                .filter_map(|f| f.parse().ok())
                .collect();
            // user nice system idle iowait irq softirq steal ...
            let total: u64 = fields.iter().sum();
            let idle = fields.get(3).copied().unwrap_or(0) + fields.get(4).copied().unwrap_or(0);
            times.total_ticks = total;
            times.used_ticks = total.saturating_sub(idle);
        } else if line.starts_with("cpu") {
            times.num_cpus += 1;
        }
    }
    times
}

pub fn current_cpu_times() -> CpuTimes {
    match fs::read_to_string("/proc/stat") {
        Ok(content) => read_cpu_times(&content),
        Err(_) => CpuTimes::default(),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MemInfo {
    pub total_bytes: u64,
    pub used_bytes: u64,
}

/// Parses /proc/meminfo; used = MemTotal - MemAvailable.
pub fn read_meminfo(content: &str) -> MemInfo {
    let mut total = 0u64;
    let mut available = 0u64;
    for line in content.lines() {
        let mut parts = line.split_ascii_whitespace();
        match parts.next() {
            Some("MemTotal:") => total = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            Some("MemAvailable:") => {
                available = parts.next().and_then(|v| v.parse().ok()).unwrap_or(0)
            }
            _ => {}
        }
    }
    MemInfo {
        total_bytes: total * 1024,
        used_bytes: total.saturating_sub(available) * 1024,
    }
}

pub fn current_meminfo() -> MemInfo {
    match fs::read_to_string("/proc/meminfo") {
        Ok(content) => read_meminfo(&content),
        Err(_) => MemInfo::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_stat() {
        let content = "cpu  100 0 50 800 50 0 0 0 0 0\ncpu0 50 0 25 400 25 0 0 0 0 0\ncpu1 50 0 25 400 25 0 0 0 0 0\nintr 12345\n";
        let t = read_cpu_times(content);
        assert_eq!(t.num_cpus, 2);
        assert_eq!(t.total_ticks, 1000);
        assert_eq!(t.used_ticks, 150); // total - idle(800) - iowait(50)
    }

    #[test]
    fn parses_meminfo() {
        let content = "MemTotal:       16384000 kB\nMemFree:         1024000 kB\nMemAvailable:    8192000 kB\n";
        let m = read_meminfo(content);
        assert_eq!(m.total_bytes, 16_384_000 * 1024);
        assert_eq!(m.used_bytes, (16_384_000 - 8_192_000) * 1024);
    }
}
