//! Real-time monitoring loop. Emits a `metrics` event ~1/s with CPU, RAM,
//! disk, network and top-process data. Stoppable via shared AtomicBool.

use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::{Disks, Networks, System};
use tauri::{AppHandle, Emitter};

pub struct MonitorState {
    /// True while a monitor loop is running (informational).
    pub running: Arc<AtomicBool>,
    /// The running loop: its join handle plus its OWN stop flag. Both
    /// start() and stop() hold this mutex for their whole critical section
    /// (check, spawn/store, signal, join), and every loop gets a fresh flag,
    /// so a concurrent stop/start can never leave two loops emitting
    /// metrics or revive a loop that was told to stop.
    thread: Mutex<Option<(std::thread::JoinHandle<()>, Arc<AtomicBool>)>>,
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
    // Poison recovery: a panicked monitor thread must not brick the monitor.
    let mut slot = st.thread.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((h, _)) = slot.as_ref() {
        if !h.is_finished() {
            return Ok(()); // already running
        }
    }
    // A finished loop (panicked) is reaped before a new one starts.
    if let Some((h, _)) = slot.take() {
        let _ = h.join();
    }
    let flag = Arc::new(AtomicBool::new(true));
    let running = flag.clone();
    // Builder::spawn returns io::Result, on failure we must reset the flag,
    // otherwise every later start() no-ops and the monitor is dead forever.
    let handle = std::thread::Builder::new().spawn(move || {
        let mut sys = System::new_all();
        let mut networks = Networks::new_with_refreshed_list();
        let mut disks = Disks::new_with_refreshed_list();
        // Live clock via "% Processor Performance": sysinfo/WMI only report
        // the static base clock on Windows.
        let freq_counter = crate::pdh::PerfCounter::new(crate::pdh::CPU_PERFORMANCE);
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
            let freq_mhz = freq_counter
                .as_ref()
                .and_then(|f| f.read())
                .map(|pct| (base_mhz as f64 * pct / 100.0).round() as u64)
                .unwrap_or(base_mhz);

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
    *slot = Some((handle, flag));
    st.running.store(true, Ordering::SeqCst);
    Ok(())
}

pub fn stop(st: &MonitorState) {
    // Hold the slot lock across signal + join so a concurrent start() waits
    // until this loop is really gone. Poison recovery instead of unwrap: a
    // panicked monitor thread must not crash the app on stop.
    let mut slot = st.thread.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((h, flag)) = slot.take() {
        flag.store(false, Ordering::SeqCst);
        let _ = h.join();
    }
    st.running.store(false, Ordering::SeqCst);
}
