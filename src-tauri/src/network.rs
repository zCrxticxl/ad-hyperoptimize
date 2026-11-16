//! Explicitly started, bounded network measurements. No settings are changed.
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

struct Job {
    id: String,
    cancel: Arc<AtomicBool>,
    done: bool,
    result: Option<Value>,
    error: Option<String>,
}
static JOB: Mutex<Option<Job>> = Mutex::new(None);

pub fn start(load: bool) -> Result<String, String> {
    let mut slot = JOB.lock().map_err(|e| e.to_string())?;
    if slot.as_ref().is_some_and(|job| !job.done) {
        return Err("A network test is already running".into());
    }
    let id = format!("net-{:032x}", rand::random::<u128>());
    let cancel = Arc::new(AtomicBool::new(false));
    *slot = Some(Job {
        id: id.clone(),
        cancel: cancel.clone(),
        done: false,
        result: None,
        error: None,
    });
    let worker_id = id.clone();
    if let Err(error) = std::thread::Builder::new()
        .name("network-diagnostic".into())
        .spawn(move || {
            let result = std::panic::catch_unwind(|| {
                let script = include_str!("network_probe.ps1")
                    .replace("__LOAD__", if load { "$true" } else { "$false" });
                let output = crate::ps::run_cancellable(&script, &cancel)?;
                let mut value: Value = serde_json::from_str(output.trim())
                    .map_err(|e| format!("Network result: {e}"))?;
                for phase in ["idle", "loaded"] {
                    if let Some(samples) = value["measurements"][phase]["samples"].as_array() {
                        let mut targets: Vec<String> = Vec::new();
                        for sample in samples {
                            if let Some(target) = sample["target"].as_str() {
                                if !targets.iter().any(|t| t == target) {
                                    targets.push(target.into());
                                }
                            }
                        }
                        let stats: Vec<_> = targets
                            .iter()
                            .map(|target| {
                                let selected: Vec<_> = samples
                                    .iter()
                                    .filter(|s| {
                                        s["target"] == *target
                                            && (phase == "idle" || s["loadActive"] == true)
                                    })
                                    .collect();
                                summarize(target, &selected)
                            })
                            .collect();
                        value["measurements"][phase]["stats"] = json!(stats);
                    }
                }
                Ok::<_, String>(value)
            })
            .unwrap_or_else(|_| Err("Network worker panicked".into()));
            match JOB.lock() {
                Ok(mut slot) => {
                    if let Some(job) = slot.as_mut().filter(|j| j.id == worker_id) {
                        job.done = true;
                        if job.cancel.load(Ordering::SeqCst) {
                            job.error = Some("cancelled".into());
                        } else {
                            match result {
                                Ok(value) => job.result = Some(value),
                                Err(error) => job.error = Some(error),
                            }
                        }
                    }
                }
                Err(error) => eprintln!("network job state: {error}"),
            }
        })
    {
        *slot = None;
        return Err(error.to_string());
    }
    Ok(id)
}

pub fn status(id: &str) -> Result<Value, String> {
    let slot = JOB.lock().map_err(|e| e.to_string())?;
    let job = slot
        .as_ref()
        .filter(|j| j.id == id)
        .ok_or("Unknown network test")?;
    Ok(json!({ "done": job.done, "result": job.result, "error": job.error }))
}
pub fn cancel(id: &str) -> Result<(), String> {
    let slot = JOB.lock().map_err(|e| e.to_string())?;
    let job = slot
        .as_ref()
        .filter(|j| j.id == id)
        .ok_or("Unknown network test")?;
    job.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

fn summarize(target: &str, samples: &[&Value]) -> Value {
    let mut rtts = Vec::new();
    let mut deltas = Vec::new();
    let mut previous: Option<f64> = None;
    let mut errors = 0;
    for sample in samples {
        let success = sample["status"] == "Success";
        let rtt = if success { sample["ms"].as_f64() } else { None };
        if sample["status"]
            .as_str()
            .is_some_and(|s| s.starts_with("LocalError:"))
        {
            errors += 1;
        }
        if let Some(ms) = rtt {
            rtts.push(ms);
            if let Some(prev) = previous {
                deltas.push((ms - prev).abs());
            }
        }
        // A missing reply breaks the consecutive-pair jitter series.
        previous = rtt;
    }
    let sent = samples.len().saturating_sub(errors);
    let mean = |xs: &[f64]| {
        if xs.is_empty() {
            None
        } else {
            Some(xs.iter().sum::<f64>() / xs.len() as f64)
        }
    };
    json!({ "target": target, "sent": sent, "received": rtts.len(), "errors": errors,
        "lossPct": if sent == 0 { None } else { Some((sent - rtts.len()) as f64 * 100.0 / sent as f64) },
        "avgMs": mean(&rtts), "maxMs": rtts.iter().copied().reduce(f64::max), "jitterMs": mean(&deltas) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loss_and_jitter_preserve_gaps_and_unknowns() {
        let data = [
            json!({"status":"Success", "ms":10}),
            json!({"status":"Success", "ms":14}),
            json!({"status":"TimedOut"}),
            json!({"status":"Success", "ms":30}),
            json!({"status":"LocalError: denied"}),
        ];
        let result = summarize("test", &data.iter().collect::<Vec<_>>());
        assert_eq!(result["sent"], 4);
        assert_eq!(result["errors"], 1);
        assert_eq!(result["lossPct"], 25.0);
        assert_eq!(result["jitterMs"], 4.0);
        let none = summarize("test", &[&json!({"status":"TimedOut"})]);
        assert_eq!(none["lossPct"], 100.0);
        assert!(none["avgMs"].is_null());
        assert!(none["jitterMs"].is_null());
        assert!(summarize("test", &[])["lossPct"].is_null());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Explicit read-only ICMP test against gateway and public resolvers"]
    fn read_only_network_smoke() {
        let id = start(false).unwrap();
        assert!(start(false).is_err(), "must reject concurrent runs");
        assert!(cancel("not-this-run").is_err());
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(65);
        loop {
            let value = status(&id).unwrap();
            if value["done"] == true {
                assert!(value["error"].is_null(), "{value}");
                assert!(value["result"]["measurements"]["loaded"].is_null());
                assert_eq!(value["result"]["measurements"]["downloadBytes"], 0);
                assert!(
                    value["result"]["measurements"]["idle"]["stats"]
                        .as_array()
                        .unwrap()
                        .len()
                        >= 2
                );
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        let second = start(false).unwrap();
        cancel(&second).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let value = status(&second).unwrap();
            if value["done"] == true {
                assert_eq!(value["error"], "cancelled");
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }
}
