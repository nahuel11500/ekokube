//! Parses Kubernetes resource quantities ("100m", "2", "1Gi", "1500M", ...).

/// Parses a CPU quantity into millicores.
pub fn parse_cpu_millis(q: &str) -> Option<u64> {
    let q = q.trim();
    if let Some(m) = q.strip_suffix('m') {
        return m.parse::<u64>().ok();
    }
    let cores: f64 = q.parse().ok()?;
    Some((cores * 1000.0).round() as u64)
}

/// Parses a memory quantity into bytes.
pub fn parse_mem_bytes(q: &str) -> Option<u64> {
    let q = q.trim();
    let suffixes: [(&str, f64); 11] = [
        ("Ki", 1024.0),
        ("Mi", 1024.0 * 1024.0),
        ("Gi", 1024.0 * 1024.0 * 1024.0),
        ("Ti", 1024.0f64.powi(4)),
        ("Pi", 1024.0f64.powi(5)),
        ("k", 1e3),
        ("M", 1e6),
        ("G", 1e9),
        ("T", 1e12),
        ("P", 1e15),
        ("m", 1e-3), // milli-bytes exist in the API (rare)
    ];
    for (suffix, factor) in suffixes {
        if let Some(num) = q.strip_suffix(suffix) {
            let value: f64 = num.parse().ok()?;
            return Some((value * factor).round() as u64);
        }
    }
    let value: f64 = q.parse().ok()?;
    Some(value.round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_quantities() {
        assert_eq!(parse_cpu_millis("100m"), Some(100));
        assert_eq!(parse_cpu_millis("1"), Some(1000));
        assert_eq!(parse_cpu_millis("2.5"), Some(2500));
        assert_eq!(parse_cpu_millis("0.1"), Some(100));
        assert_eq!(parse_cpu_millis("bogus"), None);
    }

    #[test]
    fn mem_quantities() {
        assert_eq!(parse_mem_bytes("128Mi"), Some(128 * 1024 * 1024));
        assert_eq!(parse_mem_bytes("1Gi"), Some(1024 * 1024 * 1024));
        assert_eq!(parse_mem_bytes("1500M"), Some(1_500_000_000));
        assert_eq!(parse_mem_bytes("64Ki"), Some(65536));
        assert_eq!(parse_mem_bytes("1000"), Some(1000));
        assert_eq!(parse_mem_bytes("1.5Gi"), Some(1_610_612_736));
        assert_eq!(parse_mem_bytes("bogus"), None);
    }
}
