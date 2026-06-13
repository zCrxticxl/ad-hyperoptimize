//! Built-in micro-benchmarks: CPU (SHA-256 throughput, single & all-core),
//! memory copy bandwidth, sequential disk write/read. Results persist for
//! before/after comparisons.

use rayon::prelude::*;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write as IoWrite};
use std::time::Instant;

const BUF_MB: usize = 8;
const CPU_ITERS: usize = 24;

fn hash_pass(buf: &[u8], iters: usize) -> f64 {
    let start = Instant::now();
    for _ in 0..iters {
        let mut h = Sha256::new();
        h.update(buf);
        std::hint::black_box(h.finalize());
    }
    (iters * buf.len()) as f64 / 1e6 / start.elapsed().as_secs_f64() // MB/s
}

pub fn cpu() -> Value {
    let buf = vec![0xA5u8; BUF_MB * 1024 * 1024];
    let single = hash_pass(&buf, CPU_ITERS);
    let threads = rayon::current_num_threads();
    let start = Instant::now();
    let total_mb: f64 = (0..threads)
        .into_par_iter()
        .map(|_| {
            let local = vec![0x5Au8; BUF_MB * 1024 * 1024];
            for _ in 0..CPU_ITERS {
                let mut h = Sha256::new();
                h.update(&local);
                std::hint::black_box(h.finalize());
            }
            (CPU_ITERS * local.len()) as f64 / 1e6
        })
        .sum();
    let multi = total_mb / start.elapsed().as_secs_f64();
    json!({
        "kind": "cpu",
        "singleMBs": single.round(),
        "multiMBs": multi.round(),
        "threads": threads,
        "scaling": (multi / single * 10.0).round() / 10.0
    })
}

pub fn memory() -> Value {
    const SZ: usize = 256 * 1024 * 1024;
    let src = vec![0x3Cu8; SZ];
    let mut dst = vec![0u8; SZ];
    // warmup
    dst.copy_from_slice(&src);
    let start = Instant::now();
    let passes = 6;
    for _ in 0..passes {
        dst.copy_from_slice(&src);
        std::hint::black_box(&dst);
    }
    let gbs = (SZ * passes) as f64 / 1e9 / start.elapsed().as_secs_f64();
    json!({ "kind": "memory", "copyGBs": (gbs * 100.0).round() / 100.0, "bufferMB": SZ / 1024 / 1024 })
}

pub fn disk() -> Value {
    const SZ: usize = 256 * 1024 * 1024;
    const CHUNK: usize = 4 * 1024 * 1024;
    let path = std::env::temp_dir().join("pcopt_bench.tmp");
    let data = vec![0x7Eu8; CHUNK];

    let write_mbs = (|| -> Result<f64, std::io::Error> {
        let mut f = fs::File::create(&path)?;
        let start = Instant::now();
        for _ in 0..(SZ / CHUNK) {
            f.write_all(&data)?;
        }
        f.sync_all()?;
        Ok(SZ as f64 / 1e6 / start.elapsed().as_secs_f64())
    })();

    let read_mbs = (|| -> Result<f64, std::io::Error> {
        let mut f = fs::File::open(&path)?;
        let mut buf = vec![0u8; CHUNK];
        let start = Instant::now();
        let mut total = 0usize;
        loop {
            let n = f.read(&mut buf)?;
            if n == 0 {
                break;
            }
            total += n;
            std::hint::black_box(&buf[..n]);
        }
        Ok(total as f64 / 1e6 / start.elapsed().as_secs_f64())
    })();

    let _ = fs::remove_file(&path);
    json!({
        "kind": "disk",
        "seqWriteMBs": write_mbs.map(|v| v.round()).unwrap_or(-1.0),
        "seqReadMBs": read_mbs.map(|v| v.round()).unwrap_or(-1.0),
        "note": "Sequential, 256MB, temp drive. Read may exceed media speed due to OS cache."
    })
}

fn history_path() -> std::path::PathBuf {
    crate::safety::app_data_dir().join("bench_history.json")
}

pub fn run(kind: &str) -> Result<Value, String> {
    let mut result = match kind {
        "cpu" => cpu(),
        "memory" => memory(),
        "disk" => disk(),
        _ => return Err(format!("unknown benchmark '{kind}'")),
    };
    result["time"] = json!(chrono::Local::now().to_rfc3339());
    let mut hist: Vec<Value> = fs::read_to_string(history_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    hist.push(result.clone());
    let _ = fs::write(history_path(), serde_json::to_string_pretty(&hist).unwrap_or_default());
    Ok(result)
}

pub fn history() -> Value {
    fs::read_to_string(history_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!([]))
}
