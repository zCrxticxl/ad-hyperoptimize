//! Real-time monitoring loop. Emits a `metrics` event ~1/s with CPU, RAM,
//! disk, network and top-process data. Stoppable via shared AtomicBool.

use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::{Disks, Networks, System};
use tauri::{AppHandle, Emitter};

/// Live CPU frequency via PDH "% Processor Performance" (locale-independent
/// thanks to PdhAddEnglishCounter). sysinfo/WMI only report the static base
/// clock on Windows; this counter reflects real turbo/downclock state.
#[cfg(windows)]
mod freq {
    use std::ffi::c_void;

    type HQuery = *mut c_void;
    type HCounter = *mut c_void;

    #[repr(C)]
    struct FmtValue {
        c_status: u32,
        _pad: u32,
        double_value: f64,
    }

    const PDH_FMT_DOUBLE: u32 = 0x0000_0200;

    #[link(name = "pdh")]
    extern "system" {
        fn PdhOpenQueryW(src: *const u16, user: usize, q: *mut HQuery) -> i32;
        fn PdhAddEnglishCounterW(q: HQuery, path: *const u16, user: usize, c: *mut HCounter)
            -> i32;
        fn PdhCollectQueryData(q: HQuery) -> i32;
        fn PdhGetFormattedCounterValue(
            c: HCounter,
            fmt: u32,
            ctype: *mut u32,
            val: *mut FmtValue,
        ) -> i32;
        fn PdhCloseQuery(q: HQuery) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub struct PerfCounter {
        query: HQuery,
        counter: HCounter,
    }
    // Raw handles are only used from the monitor thread.
    unsafe impl Send for PerfCounter {}

    impl PerfCounter {
        pub fn new() -> Option<Self> {
            unsafe {
                let mut q: HQuery = std::ptr::null_mut();
                if PdhOpenQueryW(std::ptr::null(), 0, &mut q) != 0 {
                    return None;
                }
                let path = wide("\\Processor Information(_Total)\\% Processor Performance");
                let mut c: HCounter = std::ptr::null_mut();
                if PdhAddEnglishCounterW(q, path.as_ptr(), 0, &mut c) != 0 {
                    PdhCloseQuery(q);
                    return None;
                }
                PdhCollectQueryData(q); // prime; rates need two samples
                Some(Self {
                    query: q,
                    counter: c,
                })
            }
        }

        /// % of nominal frequency (>100 = turbo). Call once per tick.
        pub fn read_pct(&self) -> Option<f64> {
            unsafe {
                if PdhCollectQueryData(self.query) != 0 {
                    return None;
                }
                let mut val = FmtValue {
                    c_status: 0,
                    _pad: 0,
                    double_value: 0.0,
                };
                let mut ctype = 0u32;
                if PdhGetFormattedCounterValue(self.counter, PDH_FMT_DOUBLE, &mut ctype, &mut val)
                    != 0
                {
                    return None;
                }
                Some(val.double_value)
            }
        }
    }

    impl Drop for PerfCounter {
        fn drop(&mut self) {
            unsafe { PdhCloseQuery(self.query) };
        }
    }
}

pub struct MonitorState {
    pub running: Arc<AtomicBool>,
    /// Join handle of the monitor thread — stop() joins it so a
    /// stop→start sequence can never leave two loops emitting metrics.
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl Default for MonitorState {
    fn default() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        }
    }
}

pub fn start(app: AppHandle, st: &MonitorState) -> Result<(), String> {
    if st.running.swap(true, Ordering::SeqCst) {
        return Ok(()); // already running
    }
    let running = st.running.clone();
    // Builder::spawn returns io::Result — on failure we must reset the flag,
    // otherwise every later start() no-ops and the monitor is dead forever.
    let handle = std::thread::Builder::new().spawn(move || {
        let mut sys = System::new_all();
        let mut networks = Networks::new_with_refreshed_list();
        let mut disks = Disks::new_with_refreshed_list();
        #[cfg(windows)]
        let freq_counter = freq::PerfCounter::new();
        // First CPU sample is meaningless; prime it.
        sys.refresh_cpu();
        std::thread::sleep(Duration::from_millis(400));

        while running.load(Ordering::SeqCst) {
            sys.refresh_cpu();
            sys.refresh_memory();
            sys.refresh_processes();
            networks.refresh();
            disks.refresh();

            let per_core: Vec<f32> = sys.cpus().iter().map(|c| c.cpu_usage()).collect();
            let cpu_total = per_core.iter().sum::<f32>() / per_core.len().max(1) as f32;
            let base_mhz = sys.cpus().first().map(|c| c.frequency()).unwrap_or(0);
            // Effective clock = base × "% Processor Performance" (turbo > 100%).
            #[cfg(windows)]
            let freq_mhz = freq_counter
                .as_ref()
                .and_then(|f| f.read_pct())
                .map(|pct| (base_mhz as f64 * pct / 100.0).round() as u64)
                .unwrap_or(base_mhz);
            #[cfg(not(windows))]
            let freq_mhz = base_mhz;

            let (mut rx, mut tx) = (0u64, 0u64);
            for n in networks.values() {
                rx += n.received();
                tx += n.transmitted();
            }

            let mut procs: Vec<_> = sys
                .processes()
                .values()
                .map(|p| (p.name().to_string(), p.cpu_usage(), p.memory()))
                .collect();
            procs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let top_cpu: Vec<_> = procs
                .iter()
                .take(6)
                .map(|(n, c, m)| json!({ "name": n, "cpu": (*c * 10.0).round() / 10.0, "memMb": m / 1_048_576 }))
                .collect();
            procs.sort_by_key(|p| std::cmp::Reverse(p.2));
            let top_mem: Vec<_> = procs
                .iter()
                .take(6)
                .map(|(n, c, m)| json!({ "name": n, "cpu": (*c * 10.0).round() / 10.0, "memMb": m / 1_048_576 }))
                .collect();

            let disk_info: Vec<_> = disks
                .iter()
                .map(|d| {
                    json!({
                        "name": d.mount_point().to_string_lossy(),
                        "totalGb": d.total_space() as f64 / 1e9,
                        "freeGb": d.available_space() as f64 / 1e9
                    })
                })
                .collect();

            let payload = json!({
                "t": chrono::Local::now().format("%H:%M:%S").to_string(),
                "cpuTotal": (cpu_total * 10.0).round() / 10.0,
                "perCore": per_core,
                "freqMhz": freq_mhz,
                "memUsedMb": sys.used_memory() / 1_048_576,
                "memTotalMb": sys.total_memory() / 1_048_576,
                "netRxKbs": rx as f64 / 1024.0,
                "netTxKbs": tx as f64 / 1024.0,
                "disks": disk_info,
                "topCpu": top_cpu,
                "topMem": top_mem,
            });
            let _ = app.emit("metrics", payload);
            // Interruptible sleep: stop() joins the thread, so it must not
            // block for the full second while the loop is between iterations.
            for _ in 0..10 {
                if !running.load(Ordering::SeqCst) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
    let handle = match handle {
        Ok(h) => h,
        Err(e) => {
            st.running.store(false, Ordering::SeqCst);
            return Err(format!("failed to start monitor thread: {e}"));
        }
    };
    *st.thread
        .lock()
        .map_err(|_| "monitor lock poisoned".to_string())? = Some(handle);
    Ok(())
}

pub fn stop(st: &MonitorState) {
    st.running.store(false, Ordering::SeqCst);
    let handle = st.thread.lock().unwrap().take();
    if let Some(h) = handle {
        let _ = h.join();
    }
}
