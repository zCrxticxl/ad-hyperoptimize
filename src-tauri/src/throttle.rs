//! Throttling detection: is the CPU or GPU running below what it could right
//! now, and why. Read-only.
//!
//! CPU: "% Performance Limit" (100 = no cap; lower = capped by power,
//! temperature or firmware). GPU (NVIDIA only): the driver's own clock event
//! reasons from nvidia-smi. Other GPUs expose no comparable public interface.

use serde_json::{json, Value};

/// One GPU reading from nvidia-smi.
#[derive(Debug, Clone, PartialEq)]
pub struct GpuSample {
    pub temp_c: Option<f64>,
    pub power_w: Option<f64>,
    pub power_limit_w: Option<f64>,
    pub clock_mhz: Option<f64>,
    pub max_clock_mhz: Option<f64>,
    pub util_pct: Option<f64>,
    /// Active clock event reasons that lower clocks (idle excluded).
    pub reasons: Vec<&'static str>,
}

/// NVML clock event reason bits that mean "clocks are held down".
/// Idle (0x1), application clocks (0x2), sync boost (0x10) and display clock
/// (0x100) are not throttling.
const REASONS: &[(u64, &str)] = &[
    (0x4, "powerCap"),
    (0x8, "hwSlowdown"),
    (0x20, "thermal"),
    (0x40, "hwThermal"),
    (0x80, "powerBrake"),
];

/// Severity of a reason: power cap under load is how GPU Boost normally
/// works (the card uses its whole power budget); thermal and hardware
/// slowdowns are real problems.
pub fn severity(reason: &str) -> &'static str {
    match reason {
        "powerCap" => "info",
        "thermal" => "warn",
        _ => "bad",
    }
}

pub fn reasons_from_mask(mask: u64) -> Vec<&'static str> {
    REASONS
        .iter()
        .filter(|(bit, _)| mask & bit != 0)
        .map(|(_, name)| *name)
        .collect()
}

const GPU_FIELDS: &str =
    "temperature.gpu,power.draw,power.limit,clocks.gr,clocks.max.gr,utilization.gpu";

/// Parses one `--format=csv,noheader,nounits` line of GPU_FIELDS plus the
/// reasons mask (hex).
pub fn parse_gpu_line(line: &str) -> Option<GpuSample> {
    let f: Vec<&str> = line.split(',').map(str::trim).collect();
    if f.len() != 7 {
        return None;
    }
    let num = |s: &str| s.parse::<f64>().ok();
    let mask = u64::from_str_radix(f[6].trim_start_matches("0x"), 16).ok()?;
    Some(GpuSample {
        temp_c: num(f[0]),
        power_w: num(f[1]),
        power_limit_w: num(f[2]),
        clock_mhz: num(f[3]),
        max_clock_mhz: num(f[4]),
        util_pct: num(f[5]),
        reasons: reasons_from_mask(mask),
    })
}

/// Reads the first NVIDIA GPU, or None (no NVIDIA GPU / nvidia-smi missing).
#[cfg(windows)]
pub fn gpu_sample() -> Option<GpuSample> {
    // Absolute path: the elevated app must not run a nvidia-smi from PATH.
    let smi = format!(r"{}\nvidia-smi.exe", crate::ps::system_dir());
    if !std::path::Path::new(&smi).is_file() {
        return None;
    }
    // Drivers since 535 call the field clocks_event_reasons; older ones
    // clocks_throttle_reasons.
    for reasons in [
        "clocks_event_reasons.active",
        "clocks_throttle_reasons.active",
    ] {
        let query = format!("--query-gpu={GPU_FIELDS},{reasons}");
        if let Ok((status, out, _)) =
            crate::ps::exec_capture(&smi, &[&query, "--format=csv,noheader,nounits"])
        {
            if status.success() {
                if let Some(s) = out.lines().next().and_then(parse_gpu_line) {
                    return Some(s);
                }
            }
        }
    }
    None
}

#[cfg(not(windows))]
pub fn gpu_sample() -> Option<GpuSample> {
    None
}

pub fn gpu_json(g: &GpuSample) -> Value {
    json!({
        "tempC": g.temp_c,
        "powerW": g.power_w,
        "powerLimitW": g.power_limit_w,
        "clockMhz": g.clock_mhz,
        "maxClockMhz": g.max_clock_mhz,
        "utilPct": g.util_pct,
        "reasons": g.reasons.iter().map(|r| json!({ "id": r, "severity": severity(r) })).collect::<Vec<_>>(),
    })
}

/// CPU is capped when the allowed share is clearly below 100 %.
pub fn cpu_limited(limit_pct: Option<f64>) -> bool {
    limit_pct.is_some_and(|l| l < 99.0)
}

/// One snapshot for the live card (takes ~0.3 s for the CPU counters).
pub fn snapshot() -> Value {
    let limit = crate::pdh::PerfCounter::new(crate::pdh::CPU_PERFORMANCE_LIMIT);
    let perf = crate::pdh::PerfCounter::new(crate::pdh::CPU_PERFORMANCE);
    std::thread::sleep(std::time::Duration::from_millis(300));
    let limit_pct = limit.as_ref().and_then(|c| c.read());
    let perf_pct = perf.as_ref().and_then(|c| c.read());
    let gpu = gpu_sample();
    json!({
        "time": chrono::Local::now().to_rfc3339(),
        "cpu": {
            "limitPct": limit_pct,
            "perfPct": perf_pct,
            "limited": cpu_limited(limit_pct),
        },
        "gpu": gpu.as_ref().map(gpu_json),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_smi_line() {
        let g = parse_gpu_line("71, 312.45, 320.00, 2745, 3105, 99, 0x0000000000000004").unwrap();
        assert_eq!(g.temp_c, Some(71.0));
        assert_eq!(g.power_limit_w, Some(320.0));
        assert_eq!(g.reasons, vec!["powerCap"]);
        let n = parse_gpu_line("40, [N/A], [N/A], 210, 3105, 0, 0x0000000000000001").unwrap();
        assert_eq!(n.power_w, None);
        assert!(n.reasons.is_empty(), "idle is not throttling");
        assert!(parse_gpu_line("garbage").is_none());
    }

    #[test]
    fn reason_bits_and_severity() {
        assert_eq!(reasons_from_mask(0x60), vec!["thermal", "hwThermal"]);
        assert_eq!(reasons_from_mask(0x88), vec!["hwSlowdown", "powerBrake"]);
        assert_eq!(severity("powerCap"), "info");
        assert_eq!(severity("thermal"), "warn");
        assert_eq!(severity("hwThermal"), "bad");
    }

    #[test]
    fn cpu_limit_threshold() {
        assert!(!cpu_limited(Some(100.0)));
        assert!(!cpu_limited(None));
        assert!(cpu_limited(Some(80.0)));
    }
}
