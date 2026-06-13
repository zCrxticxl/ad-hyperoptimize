//! DPC / ISR latency analysis.
//!
//! Three real measurement layers:
//! 1. Per-core DPC / interrupt pressure via WMI formatted perf counters
//!    (`Win32_PerfFormattedData_Counters_ProcessorInformation`) — class and
//!    property names are locale-independent, unlike `Get-Counter` paths.
//! 2. Execution-stall probe: a pinned spinning thread timestamps continuously;
//!    any gap between consecutive reads is time the thread was preempted by
//!    DPCs/ISRs/scheduler — the same signal LatencyMon's "highest measured
//!    interrupt to process latency" reflects.
//! 3. Deep trace: drives the built-in `wpr.exe` (Windows Performance
//!    Recorder) CPU profile for per-driver DPC attribution in WPA.

use crate::ps;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Layer 1: sample per-core DPC%/Interrupt%/DPC rate `samples` times, 1s apart.
pub fn counters(samples: u32) -> Value {
    let samples = samples.clamp(2, 30);
    let script = format!(
        "$acc=@{{}}; for($i=0;$i -lt {samples};$i++){{ \
           Get-CimInstance Win32_PerfFormattedData_Counters_ProcessorInformation | ForEach-Object {{ \
             if(-not $acc[$_.Name]){{ $acc[$_.Name]=@{{dpc=0.0;int=0.0;rate=0.0;n=0;maxDpc=0.0}} }}; \
             $a=$acc[$_.Name]; $a.dpc+=$_.PercentDPCTime; $a.int+=$_.PercentInterruptTime; \
             $a.rate+=$_.DPCRate; $a.n++; if($_.PercentDPCTime -gt $a.maxDpc){{$a.maxDpc=[double]$_.PercentDPCTime}} \
           }}; Start-Sleep -Milliseconds 900 }}; \
         $acc.GetEnumerator() | ForEach-Object {{ @{{ core=$_.Key; \
           avgDpcPct=[math]::Round($_.Value.dpc/$_.Value.n,2); \
           maxDpcPct=$_.Value.maxDpc; \
           avgIntPct=[math]::Round($_.Value.int/$_.Value.n,2); \
           avgDpcRate=[math]::Round($_.Value.rate/$_.Value.n,0) }} }}"
    );
    ps::run_json(&script).unwrap_or_else(|e| json!({ "error": e.trim() }))
}

/// Layer 2: spin-probe. Returns stall distribution in microseconds.
/// A gap > ~250µs on an idle-ish core almost always means a long DPC/ISR.
pub fn stall_probe(seconds: u32) -> Value {
    let seconds = seconds.clamp(1, 30) as u64;
    let handle = std::thread::spawn(move || {
        let mut gaps_us: Vec<f64> = Vec::with_capacity(4096);
        let end = Instant::now() + Duration::from_secs(seconds);
        let mut last = Instant::now();
        while Instant::now() < end {
            let now = Instant::now();
            let gap = now.duration_since(last).as_secs_f64() * 1e6;
            // Ignore sub-5µs gaps (loop overhead noise).
            if gap > 5.0 {
                gaps_us.push(gap);
            }
            last = now;
        }
        gaps_us
    });
    let mut gaps = handle.join().unwrap_or_default();
    if gaps.is_empty() {
        return json!({ "error": "no samples collected" });
    }
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let pct = |p: f64| gaps[((gaps.len() as f64 - 1.0) * p) as usize];
    let max = *gaps.last().unwrap();
    let over_250 = gaps.iter().filter(|g| **g > 250.0).count();
    let over_1000 = gaps.iter().filter(|g| **g > 1000.0).count();

    let verdict = if max < 250.0 {
        ("excellent", "No significant execution stalls. Drivers are well-behaved; suitable for real-time audio and competitive gaming.")
    } else if max < 1000.0 {
        ("good", "Minor stalls under 1ms — normal Windows background behavior, imperceptible in games.")
    } else if max < 4000.0 {
        ("fair", "Stalls of 1–4ms detected. Can cause occasional frametime spikes or audio crackle under load. Run a deep trace to identify the driver.")
    } else {
        ("poor", "Stalls above 4ms detected — a driver is hogging DPC/ISR time. Typical culprits: Wi-Fi/LAN drivers, audio stacks, storage drivers, GPU driver during clock changes. Run a deep trace and check the worst DPC in WPA.")
    };

    json!({
        "seconds": seconds,
        "samples": gaps.len(),
        "p50us": (pct(0.50) * 10.0).round() / 10.0,
        "p99us": (pct(0.99) * 10.0).round() / 10.0,
        "p999us": (pct(0.999) * 10.0).round() / 10.0,
        "maxUs": (max * 10.0).round() / 10.0,
        "stallsOver250us": over_250,
        "stallsOver1ms": over_1000,
        "rating": verdict.0,
        "explanation": verdict.1,
    })
}

/// Layer 3: deep ETW trace via built-in Windows Performance Recorder.
pub fn wpr_start() -> Result<String, String> {
    if !ps::is_admin() {
        return Err("Deep tracing needs administrator rights.".into());
    }
    ps::exec("wpr.exe", &["-start", "CPU", "-filemode"])
        .map(|_| "Recording kernel CPU/DPC/ISR trace… Reproduce the stutter now (keep it under ~30s — traces grow fast), then press Stop.".into())
        .map_err(|e| {
            if e.contains("already") {
                "A WPR trace is already running — press Stop to finish it.".into()
            } else {
                e
            }
        })
}

pub fn wpr_stop() -> Result<Value, String> {
    let path = crate::safety::app_data_dir()
        .join("reports")
        .join(format!("dpc-trace-{}.etl", chrono::Local::now().format("%Y%m%d-%H%M%S")));
    ps::exec("wpr.exe", &["-stop", &path.to_string_lossy(), "PCOptSuite DPC trace"])?;
    Ok(json!({
        "etlPath": path.to_string_lossy(),
        "next": "Open this .etl in Windows Performance Analyzer (Microsoft Store: 'Windows Performance Analyzer'), add the 'DPC/ISR Duration by Module' graph — the top module is your latency offender."
    }))
}

pub fn wpr_cancel() -> Result<String, String> {
    ps::exec("wpr.exe", &["-cancel"]).map(|_| "Trace cancelled, nothing written.".into())
}
