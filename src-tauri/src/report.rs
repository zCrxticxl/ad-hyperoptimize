//! Report generation: standalone dark-theme HTML + raw JSON export, written
//! to %APPDATA%\PCOptSuite\reports. (HTML prints cleanly to PDF via the
//! system print dialog.)

use serde_json::Value;
use std::fs;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn sev_label(s: u64) -> (&'static str, &'static str) {
    match s {
        5 => ("CRITICAL", "#ff4d6d"),
        4 => ("HIGH", "#ff8c42"),
        3 => ("MEDIUM", "#ffd166"),
        2 => ("LOW", "#4cc9f0"),
        _ => ("INFO", "#8d99ae"),
    }
}

pub fn generate(scan: &Value, analysis: &Value, security: &Value, history: &Value) -> Result<Value, String> {
    let ts = chrono::Local::now();
    let dir = crate::safety::app_data_dir().join("reports");
    let html_path = dir.join(format!("report-{}.html", ts.format("%Y%m%d-%H%M%S")));
    let json_path = dir.join(format!("report-{}.json", ts.format("%Y%m%d-%H%M%S")));

    let full = serde_json::json!({
        "generated": ts.to_rfc3339(),
        "tool": "AD HyperOptimize v0.1.0",
        "analysis": analysis, "scan": scan, "security": security, "changeHistory": history,
    });
    fs::write(&json_path, serde_json::to_string_pretty(&full).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;

    let score = analysis["healthScore"].as_i64().unwrap_or(0);
    let summary = analysis["summary"].as_str().unwrap_or("");
    let empty = vec![];
    let findings = analysis["findings"].as_array().unwrap_or(&empty);

    let mut rows = String::new();
    for fd in findings {
        let (label, color) = sev_label(fd["severity"].as_u64().unwrap_or(1));
        rows.push_str(&format!(
            "<tr><td><span class=\"sev\" style=\"background:{color}\">{label}</span></td>\
             <td><b>{}</b><br><span class=\"muted\">{}</span></td><td>{}</td></tr>",
            esc(fd["title"].as_str().unwrap_or("")),
            esc(fd["detail"].as_str().unwrap_or("")),
            esc(fd["recommendation"].as_str().unwrap_or(""))
        ));
    }

    let os = format!(
        "{} (build {}) · {}",
        scan["os"]["Caption"].as_str().unwrap_or("?"),
        scan["os"]["BuildNumber"].as_str().unwrap_or("?"),
        scan["os"]["OSArchitecture"].as_str().unwrap_or("?")
    );
    let cpu = scan["cpu"]["Name"].as_str()
        .or_else(|| scan["cpu"][0]["Name"].as_str())
        .unwrap_or("?");

    let hist_rows: String = history.as_array().map(|h| h.iter().rev().take(30).map(|e| format!(
        "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
        esc(e["time"].as_str().unwrap_or("")),
        esc(e["tweak_name"].as_str().unwrap_or("")),
        if e["reverted"].as_bool() == Some(true) { "reverted" } else { "applied" }
    )).collect()).unwrap_or_default();

    let html = format!(r#"<!doctype html><html><head><meta charset="utf-8">
<title>AD HyperOptimize — System Report</title><style>
body{{font-family:'Segoe UI',system-ui,sans-serif;background:#0d1117;color:#e6edf3;margin:0;padding:40px;max-width:1000px;margin:auto}}
h1{{font-size:26px}} h2{{font-size:18px;border-bottom:1px solid #21262d;padding-bottom:8px;margin-top:36px}}
.muted{{color:#8b949e;font-size:13px}}
.score{{font-size:54px;font-weight:700;color:{score_color}}}
table{{width:100%;border-collapse:collapse;margin-top:12px}}
td,th{{padding:10px;border-bottom:1px solid #21262d;vertical-align:top;text-align:left;font-size:14px}}
.sev{{padding:3px 10px;border-radius:12px;color:#0d1117;font-weight:700;font-size:11px;white-space:nowrap}}
@media print{{body{{background:#fff;color:#111}} td,th{{border-color:#ddd}} .muted{{color:#555}}}}
</style></head><body>
<h1>AD <span style="color:#4f8cff">Hyper</span>Optimize — System Report</h1>
<p class="muted">Generated {generated} · {os} · {cpu}</p>
<h2>Health Score</h2><div class="score">{score}/100</div><p>{summary}</p>
<h2>Findings ({nf})</h2><table><tr><th>Severity</th><th>Issue</th><th>Recommendation</th></tr>{rows}</table>
<h2>Optimization History</h2><table><tr><th>Time</th><th>Tweak</th><th>State</th></tr>{hist_rows}</table>
<p class="muted">Full machine-readable data: {json_name}. Use the browser print dialog for a PDF copy.</p>
</body></html>"#,
        score_color = if score >= 80 { "#3fb950" } else if score >= 50 { "#ffd166" } else { "#ff4d6d" },
        generated = ts.format("%Y-%m-%d %H:%M"),
        os = esc(&os), cpu = esc(cpu), nf = findings.len(),
        json_name = json_path.file_name().unwrap_or_default().to_string_lossy(),
    );
    fs::write(&html_path, html).map_err(|e| e.to_string())?;

    Ok(serde_json::json!({
        "htmlPath": html_path.to_string_lossy(),
        "jsonPath": json_path.to_string_lossy(),
    }))
}
