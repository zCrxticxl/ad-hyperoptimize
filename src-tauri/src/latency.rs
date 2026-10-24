//! DPC / ISR latency analysis.
//!
//! Three real measurement layers:
//! 1. Per-core DPC / interrupt pressure via WMI formatted perf counters
//!    (`Win32_PerfFormattedData_Counters_ProcessorInformation`), class and
//!    property names are locale-independent, unlike `Get-Counter` paths.
//! 2. Execution-stall probe: a spinning thread timestamps continuously; any
//!    gap between consecutive reads is time the thread was preempted by
//!    DPCs/ISRs/scheduler, similar to LatencyMon's "highest measured
//!    interrupt to process latency". The thread is NOT pinned to a core (no
//!    affinity is set), so the scheduler may migrate it and a gap can also be
//!    plain preemption by other threads, not only driver DPC/ISR time.
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
/// The probe thread is unpinned; on an otherwise idle system a gap > ~250µs
/// usually means a long DPC/ISR, but under load it can be ordinary preemption.
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

    // The rating follows the single worst gap of an unpinned thread, so the
    // texts name likely causes without claiming a driver was proven guilty.
    let verdict = if max < 250.0 {
        (
            "excellent",
            "No execution stall above 250 µs during the probe.",
        )
    } else if max < 1000.0 {
        (
            "good",
            "Stalls under 1 ms, common Windows background behavior; usually imperceptible in games.",
        )
    } else if max < 4000.0 {
        ("fair", "Stalls of 1-4 ms measured. Repeated stalls of this size can cause frametime spikes or audio crackle. On a loaded system they can also be ordinary preemption: repeat the probe idle, and run a deep trace to see the cause.")
    } else {
        ("poor", "Stalls above 4 ms measured. On an idle system this usually points to a long DPC/ISR (typical: Wi-Fi/LAN, audio, storage or GPU drivers); under load it can also be ordinary preemption of the probe thread. Repeat the probe idle, and run a deep trace to see which driver, if any, is responsible.")
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
        .map(|_| "Recording kernel CPU/DPC/ISR trace… Reproduce the stutter now (keep it under ~30s, traces grow fast), then press Stop.".into())
        .map_err(|e| {
            if e.contains("already") {
                "A WPR trace is already running, press Stop to finish it.".into()
            } else {
                e
            }
        })
}

pub fn wpr_stop() -> Result<Value, String> {
    let path = crate::safety::app_data_dir().join("reports").join(format!(
        "dpc-trace-{}.etl",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    // wpr writes the trace with this process's rights into the
    // user-writable reports folder: no link in the path, no existing file.
    if crate::diskanalyzer::has_reparse_ancestor(&path) || path.symlink_metadata().is_ok() {
        return Err(format!(
            "refusing to write the trace to {}: link in the path or file exists",
            path.display()
        ));
    }
    ps::exec(
        "wpr.exe",
        &["-stop", &path.to_string_lossy(), "PCOptSuite DPC trace"],
    )?;
    Ok(json!({
        "etlPath": path.to_string_lossy(),
        "analysis": analyze_etl(&path.to_string_lossy()),
        "next": "For a timeline, open this .etl in Windows Performance Analyzer (Microsoft Store) and add the 'DPC/ISR Duration by Module' graph."
    }))
}

pub fn wpr_cancel() -> Result<String, String> {
    ps::exec("wpr.exe", &["-cancel"]).map(|_| "Trace cancelled, nothing written.".into())
}

// ── Per-driver DPC/ISR attribution from the WPR trace ────────────────────────
//
// The trace (kernel CPU profile) contains PerfInfo DPC/ISR events with the
// routine address and start time, and Image load/rundown events with each
// kernel module's address range. Each DPC/ISR is assigned to the module whose
// range contains its routine; its duration is end (event time) minus start.

#[derive(Debug, Clone, PartialEq)]
pub struct DpcEvent {
    pub routine: u64,
    pub dur_us: f64,
    pub isr: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub base: u64,
    pub size: u64,
    pub name: String,
}

/// Module that contains `addr`; `images` sorted by base.
fn module_of(images: &[Image], addr: u64) -> Option<&Image> {
    let i = images.partition_point(|m| m.base <= addr);
    let m = images.get(i.checked_sub(1)?)?;
    (addr < m.base.saturating_add(m.size)).then_some(m)
}

/// Recommendation key for a module (texts live in the UI).
pub fn advice(module: &str) -> &'static str {
    let m = module.to_ascii_lowercase();
    let has = |s: &[&str]| s.iter().any(|k| m.starts_with(k));
    if has(&["nvlddmkm"]) {
        "gpuNvidia"
    } else if has(&["amdkmdag", "atikmdag", "amdkmpfd"]) {
        "gpuAmd"
    } else if has(&["igdkmd", "igdkmdn"]) {
        "gpuIntel"
    } else if has(&["dxgkrnl", "dxgmms"]) {
        "directx"
    } else if has(&[
        "netwtw", "netwbw", "rtwlan", "athw", "qcamain", "mrvlpcie", "bcmwl", "rtkwlan", "mtkwl",
    ]) {
        "wifi"
    } else if has(&[
        "ndis", "tcpip", "netio", "rt640", "rtots", "rtgn", "e1d", "e1i", "e2f", "rtux", "killer",
        "bxvbd", "ixgbe", "aqnic",
    ]) {
        "network"
    } else if has(&[
        "storport", "stornvme", "storahci", "iastor", "amdsata", "amdraid", "nvme", "disk.sys",
        "classpnp", "rcbottom",
    ]) {
        "storage"
    } else if has(&[
        "usbxhci", "usbport", "ucx01000", "usbhub", "usbehci", "amdxhc", "asmtxhci",
    ]) {
        "usb"
    } else if has(&["acpi"]) {
        "acpi"
    } else if has(&[
        "hdaudbus",
        "portcls",
        "rtkvhd",
        "rtkhdaud",
        "cmudaxp",
        "nahimic",
        "senfilt",
        "intcaudiobus",
        "ks.sys",
        "usbaudio",
    ]) {
        "audio"
    } else if has(&["intelppm", "amdppm", "processr"]) {
        "cpuPower"
    } else if has(&["ntoskrnl", "hal.dll"]) {
        "kernel"
    } else if has(&["wdf01000", "wpprecorder"]) {
        "framework"
    } else if has(&["hidclass", "hidusb", "mouhid", "kbdhid", "i8042prt"]) {
        "input"
    } else {
        "other"
    }
}

/// Aggregates events per module, worst first.
pub fn attribute(events: &[DpcEvent], images: &mut [Image]) -> Value {
    images.sort_by_key(|m| m.base);
    let mut by: std::collections::HashMap<String, (u64, u64, f64, f64, u64, u64)> =
        std::collections::HashMap::new();
    let mut unknown = 0u64;
    for e in events {
        let Some(m) = module_of(images, e.routine) else {
            unknown += 1;
            continue;
        };
        let s = by.entry(m.name.clone()).or_default();
        if e.isr {
            s.1 += 1;
        } else {
            s.0 += 1;
        }
        s.2 += e.dur_us;
        if e.dur_us > s.3 {
            s.3 = e.dur_us;
        }
        if e.dur_us >= 500.0 {
            s.4 += 1;
        }
        if e.dur_us >= 1000.0 {
            s.5 += 1;
        }
    }
    let mut rows: Vec<Value> = by
        .into_iter()
        .map(|(name, (dpc, isr, total, max, o500, o1000))| {
            let rating = if max >= 1000.0 {
                "bad"
            } else if max >= 500.0 {
                "warn"
            } else {
                "ok"
            };
            json!({
                "module": name, "dpcs": dpc, "isrs": isr,
                "totalMs": (total / 100.0).round() / 10.0,
                "maxUs": max.round(), "over500us": o500, "over1ms": o1000,
                "rating": rating, "advice": advice(&name),
            })
        })
        .collect();
    rows.sort_by(|a, b| {
        b["maxUs"]
            .as_f64()
            .partial_cmp(&a["maxUs"].as_f64())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    rows.truncate(15);
    json!({ "events": events.len(), "unattributed": unknown, "modules": rows })
}

#[cfg(windows)]
mod etl {
    use super::{DpcEvent, Image};
    use std::ffi::c_void;

    // sizeof(EVENT_TRACE_LOGFILEW) on x64 and the field offsets used here.
    const LOGFILE_SIZE: usize = 448;
    const OFF_LOGFILE_NAME: usize = 0;
    const OFF_MODE: usize = 28;
    const OFF_HEADER: usize = 120;
    const OFF_CPU_MHZ: usize = OFF_HEADER + 52;
    const OFF_PERF_FREQ: usize = OFF_HEADER + 256;
    const OFF_CLOCK_TYPE: usize = OFF_HEADER + 272;
    const OFF_CALLBACK: usize = 424;
    const OFF_CONTEXT: usize = 440;
    const MODE_EVENT_RECORD: u32 = 0x1000_0000;
    const MODE_RAW_TIMESTAMP: u32 = 0x0000_1000;
    const INVALID: u64 = u64::MAX;

    #[repr(C, align(8))]
    struct LogFile([u8; LOGFILE_SIZE]);

    #[link(name = "advapi32")]
    extern "system" {
        fn OpenTraceW(logfile: *mut LogFile) -> u64;
        fn ProcessTrace(
            handles: *const u64,
            count: u32,
            start: *const c_void,
            end: *const c_void,
        ) -> u32;
        fn CloseTrace(handle: u64) -> u32;
    }

    fn guid(d1: u32, d2: u16, d3: u16, d4: [u8; 8]) -> [u8; 16] {
        let mut g = [0u8; 16];
        g[0..4].copy_from_slice(&d1.to_le_bytes());
        g[4..6].copy_from_slice(&d2.to_le_bytes());
        g[6..8].copy_from_slice(&d3.to_le_bytes());
        g[8..16].copy_from_slice(&d4);
        g
    }

    struct Ctx {
        perf_info: [u8; 16],
        image: [u8; 16],
        raw: Vec<(u64, i64, i64, bool)>, // routine, start, end, isr
        images: Vec<Image>,
    }

    unsafe fn rd<T: Copy>(p: *const u8, off: usize) -> T {
        std::ptr::read_unaligned(p.add(off) as *const T)
    }

    extern "system" fn callback(rec: *mut u8) {
        // No panics may cross this FFI boundary: plain reads with bounds checks.
        unsafe {
            let ctx = rd::<*mut Ctx>(rec, 104);
            if ctx.is_null() {
                return;
            }
            let ctx = &mut *ctx;
            let provider: [u8; 16] = rd(rec, 24);
            let opcode: u8 = rd(rec, 45);
            let flags: u16 = rd(rec, 4);
            let ptr = if flags & 0x0020 != 0 { 4 } else { 8 };
            let len = rd::<u16>(rec, 86) as usize;
            let data = rd::<*const u8>(rec, 96);
            if data.is_null() {
                return;
            }
            let read_ptr = |off: usize| -> Option<u64> {
                if off + ptr > len {
                    return None;
                }
                Some(if ptr == 4 {
                    rd::<u32>(data, off) as u64
                } else {
                    rd::<u64>(data, off)
                })
            };
            if provider == ctx.perf_info && matches!(opcode, 50 | 66 | 67 | 68 | 69) {
                if len < 8 + ptr {
                    return;
                }
                let start: i64 = rd(data, 0);
                let end: i64 = rd(rec, 16);
                if let Some(routine) = read_ptr(8) {
                    ctx.raw
                        .push((routine, start, end, matches!(opcode, 50 | 67)));
                }
            } else if provider == ctx.image && matches!(opcode, 3 | 4 | 10) {
                let (pid_off, name_off) = if ptr == 8 { (16, 56) } else { (8, 44) };
                if len < name_off + 2 {
                    return;
                }
                let pid: u32 = rd(data, pid_off);
                if pid != 0 {
                    return;
                }
                let (Some(base), Some(size)) = (read_ptr(0), read_ptr(ptr)) else {
                    return;
                };
                let chars = (len - name_off) / 2;
                let mut name = Vec::with_capacity(chars);
                for i in 0..chars {
                    let c: u16 = rd(data, name_off + i * 2);
                    if c == 0 {
                        break;
                    }
                    name.push(c);
                }
                let full = String::from_utf16_lossy(&name);
                let file = full.rsplit('\\').next().unwrap_or(&full).to_string();
                if !file.is_empty() && !ctx.images.iter().any(|m| m.base == base) {
                    ctx.images.push(Image {
                        base,
                        size,
                        name: file,
                    });
                }
            }
        }
    }

    pub fn read(path: &str) -> Result<(Vec<DpcEvent>, Vec<Image>), String> {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut ctx = Box::new(Ctx {
            perf_info: guid(
                0xce1d_bfb4,
                0x137e,
                0x4da6,
                [0x87, 0xb0, 0x3f, 0x59, 0xaa, 0x10, 0x2c, 0xbc],
            ),
            image: guid(
                0x2cb1_5d1d,
                0x5fc1,
                0x11d2,
                [0xab, 0xe1, 0x00, 0xa0, 0xc9, 0x11, 0xf5, 0x18],
            ),
            raw: Vec::new(),
            images: Vec::new(),
        });
        let mut lf = LogFile([0; LOGFILE_SIZE]);
        unsafe {
            let p = lf.0.as_mut_ptr();
            std::ptr::write_unaligned(p.add(OFF_LOGFILE_NAME) as *mut *const u16, wide.as_ptr());
            std::ptr::write_unaligned(
                p.add(OFF_MODE) as *mut u32,
                MODE_EVENT_RECORD | MODE_RAW_TIMESTAMP,
            );
            let cb: extern "system" fn(*mut u8) = callback;
            std::ptr::write_unaligned(
                p.add(OFF_CALLBACK) as *mut *const c_void,
                cb as *const c_void,
            );
            std::ptr::write_unaligned(p.add(OFF_CONTEXT) as *mut *mut Ctx, &mut *ctx as *mut Ctx);
            let h = OpenTraceW(&mut lf);
            if h == INVALID {
                return Err("The trace file could not be opened.".into());
            }
            let rc = ProcessTrace(&h, 1, std::ptr::null(), std::ptr::null());
            CloseTrace(h);
            if rc != 0 {
                return Err(format!("Reading the trace failed ({rc})."));
            }
            let freq: i64 = std::ptr::read_unaligned(p.add(OFF_PERF_FREQ) as *const i64);
            let clock: u32 = std::ptr::read_unaligned(p.add(OFF_CLOCK_TYPE) as *const u32);
            let mhz: u32 = std::ptr::read_unaligned(p.add(OFF_CPU_MHZ) as *const u32);
            // Raw timestamps: QPC (1), system time in 100 ns (2), CPU cycles (3).
            let ticks_per_us = match clock {
                2 => 10.0,
                3 if mhz > 0 => mhz as f64,
                _ if freq > 0 => freq as f64 / 1e6,
                _ => 10.0,
            };
            let events = ctx
                .raw
                .iter()
                .filter(|(_, s, e, _)| e >= s)
                .map(|&(routine, s, e, isr)| DpcEvent {
                    routine,
                    dur_us: (e - s) as f64 / ticks_per_us,
                    isr,
                })
                // Durations above 1 s are clock artifacts, not DPCs.
                .filter(|d| d.dur_us < 1_000_000.0)
                .collect();
            Ok((events, std::mem::take(&mut ctx.images)))
        }
    }
}

/// Per-driver DPC/ISR summary of a WPR trace.
#[cfg(windows)]
pub fn analyze_etl(path: &str) -> Value {
    match etl::read(path) {
        Ok((events, mut images)) if !events.is_empty() => attribute(&events, &mut images),
        Ok(_) => json!({ "error": "The trace contains no DPC/ISR events." }),
        Err(e) => json!({ "error": e }),
    }
}

#[cfg(not(windows))]
pub fn analyze_etl(_path: &str) -> Value {
    json!({ "error": "Windows only" })
}

#[cfg(test)]
mod attribution_tests {
    use super::*;

    #[test]
    fn events_are_assigned_to_the_containing_module() {
        let mut images = vec![
            Image {
                base: 0xfffff800_20000000,
                size: 0x100000,
                name: "nvlddmkm.sys".into(),
            },
            Image {
                base: 0xfffff800_10000000,
                size: 0x80000,
                name: "ndis.sys".into(),
            },
        ];
        let ev = |r: u64, us: f64| DpcEvent {
            routine: r,
            dur_us: us,
            isr: false,
        };
        let a = attribute(
            &[
                ev(0xfffff800_20000100, 1500.0),
                ev(0xfffff800_20000200, 50.0),
                ev(0xfffff800_10000010, 300.0),
                ev(0x1234, 10.0),
            ],
            &mut images,
        );
        assert_eq!(a["unattributed"], 1);
        assert_eq!(a["modules"][0]["module"], "nvlddmkm.sys");
        assert_eq!(a["modules"][0]["dpcs"], 2);
        assert_eq!(a["modules"][0]["maxUs"], 1500.0);
        assert_eq!(a["modules"][0]["rating"], "bad");
        assert_eq!(a["modules"][0]["advice"], "gpuNvidia");
        assert_eq!(a["modules"][1]["advice"], "network");
        assert_eq!(a["modules"][1]["rating"], "ok");
    }

    #[test]
    fn advice_keys() {
        assert_eq!(advice("Netwtw10.sys"), "wifi");
        assert_eq!(advice("rt640x64.sys"), "network");
        assert_eq!(advice("stornvme.sys"), "storage");
        assert_eq!(advice("USBXHCI.SYS"), "usb");
        assert_eq!(advice("ACPI.sys"), "acpi");
        assert_eq!(advice("RTKVHD64.sys"), "audio");
        assert_eq!(advice("something.sys"), "other");
    }
}
