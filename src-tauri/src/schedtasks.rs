//! Scheduled Tasks Manager.
//! Lists all Windows Scheduled Tasks and lets the user enable/disable them.
//! Includes a curated list of known background/telemetry tasks with descriptions.

use serde_json::{json, Value};
use std::collections::HashMap;

/// (task_path_lowercase, task_name_lowercase) → German reason string.
fn bloat_catalog() -> HashMap<(String, String), &'static str> {
    let entries: &[(&str, &str, &str)] = &[
        // Application Experience
        ("\\microsoft\\windows\\application experience\\", "microsoft compatibility appraiser",
            "Sendet App-Kompatibilitäts-Telemetrie an Microsoft. Läuft täglich und erzeugt Disk-I/O."),
        ("\\microsoft\\windows\\application experience\\", "programdataupdater",
            "Aktualisiert die Appcompat-Telemetrie-Datenbank — reine Datensammlung für Microsoft."),
        ("\\microsoft\\windows\\application experience\\", "startupapptask",
            "Scannt Autostart-Programme für Microsoft-Analyse nach dem Login."),
        // Autochk
        ("\\microsoft\\windows\\autochk\\", "proxy",
            "Leitet Autochk-Ergebnisse (Disk-Fehler) an Microsoft weiter (Telemetrie)."),
        // CEIP
        ("\\microsoft\\windows\\customer experience improvement program\\", "consolidator",
            "CEIP-Telemetrie: Sendet Systemnutzungsdaten an Microsoft. Läuft stündlich."),
        ("\\microsoft\\windows\\customer experience improvement program\\", "usbceip",
            "Sendet USB-Gerätedaten im Rahmen des Customer Experience Improvement Program."),
        // Disk
        ("\\microsoft\\windows\\diskdiagnostic\\", "microsoft-windows-diskdiagnosticdatacollector",
            "Sammelt Festplattendiagnosedaten und leitet sie an Microsoft weiter."),
        ("\\microsoft\\windows\\diskfootprint\\", "diagnostics",
            "Profilt Disk-I/O-Muster im Hintergrund — erzeugt Last ohne Nutzervorteil."),
        // WinSAT
        ("\\microsoft\\windows\\maintenance\\", "winsat",
            "Windows System Assessment Tool — führt Benchmarks im Hintergrund aus und erzeugt CPU/Disk-Last."),
        // Maps
        ("\\microsoft\\windows\\maps\\", "mapsupdatetask",
            "Lädt automatisch Offline-Karten-Updates für die Windows Maps App herunter."),
        ("\\microsoft\\windows\\maps\\", "mapstoasttask",
            "Sendet Benachrichtigungen der Windows Maps App."),
        // NetTrace
        ("\\microsoft\\windows\\nettrace\\", "gathernetworkinfo",
            "Sammelt detaillierte Netzwerkdiagnosedaten — Hintergrund-Overhead ohne erkennbaren Nutzen."),
        // Power Efficiency
        ("\\microsoft\\windows\\power efficiency diagnostics\\", "analyzesystem",
            "Führt Stromeffizienz-Analyse durch und sendet Ergebnisse an Microsoft."),
        // Family Safety
        ("\\microsoft\\windows\\shell\\", "familysafetyrefreshtask",
            "Aktualisiert Family Safety / Jugendschutz-Richtlinien vom Microsoft-Server."),
        // WER
        ("\\microsoft\\windows\\windows error reporting\\", "queuereporting",
            "Sendet gereihte Fehlerberichte an Microsoft. Unnötig wenn Windows Error Reporting deaktiviert."),
        // Windows Update
        ("\\microsoft\\windows\\windowsupdate\\", "automatic app update",
            "Aktualisiert Store-Apps automatisch im Hintergrund — separat vom normalen Windows Update."),
        ("\\microsoft\\windows\\windowsupdate\\", "scheduled start",
            "Startet Windows Update-Scans nach einem festen Zeitplan."),
        // Workplace Join
        ("\\microsoft\\windows\\workplace join\\", "automatic-device-join",
            "Registriert das Gerät automatisch im Azure AD / Workplace Join (Enterprise-Feature)."),
        // PushToInstall
        ("\\microsoft\\windows\\pushtoinstall\\", "logincheck",
            "Prüft beim Login Push-to-Install-Store-Aufgaben — irrelevant für Non-Enterprise."),
        // MUI language packs
        ("\\microsoft\\windows\\mui\\", "lpremove",
            "Entfernt nicht genutzte Sprachpakete automatisch — kann unbeabsichtigt Sprachpakete löschen."),
        // Subscription / license
        ("\\microsoft\\windows\\subscription\\", "enablelicenseacquisition",
            "Versucht Windows-Aktivierungslizenzen im Hintergrund nachzuladen."),
        ("\\microsoft\\windows\\subscription\\", "licenseacquisition",
            "Lizenz-Nachlade-Task — bei korrekt aktivierten Systemen überflüssig."),
        // Device Census
        ("\\microsoft\\windows\\device information\\", "device",
            "Sendet detaillierte Geräteinventardaten an Microsoft (Device Census)."),
        ("\\microsoft\\windows\\device information\\", "device user",
            "Sendet benutzerbezogene Gerätedaten an Microsoft."),
        // Feedback
        ("\\microsoft\\windows\\feedback\\siuf\\", "dosiffeedbacktask",
            "Windows Feedback Hub: Fragt nach Nutzerfeedback im Hintergrund."),
        ("\\microsoft\\windows\\feedback\\siuf\\", "dosiffeedbacktasknonujailbreak",
            "Feedback-Hub-Hintergrundtask für Nicht-Insider."),
        // Clip
        ("\\microsoft\\windows\\clip\\", "license validation",
            "Microsoft Store Lizenzvalidierung — läuft periodisch im Hintergrund."),
    ];

    entries
        .iter()
        .map(|(p, n, r)| ((p.to_string(), n.to_string()), *r))
        .collect()
}

pub fn list() -> Value {
    let catalog = bloat_catalog();

    let script = r#"
$tasks = Get-ScheduledTask -ErrorAction SilentlyContinue | ForEach-Object {
    [PSCustomObject]@{
        Path  = $_.TaskPath
        Name  = $_.TaskName
        State = $_.State.ToString()
    }
}
$tasks | ConvertTo-Json -Compress -Depth 2
"#;

    let raw = match crate::ps::run(script) {
        Ok(s) => s,
        Err(e) => return json!({ "error": e, "tasks": [] }),
    };

    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return json!({ "tasks": [] });
    }

    let parsed: Vec<Value> = match serde_json::from_str(trimmed) {
        Ok(Value::Array(v)) => v,
        Ok(single @ Value::Object(_)) => vec![single],
        _ => return json!({ "error": "parse error", "tasks": [] }),
    };

    let tasks: Vec<Value> = parsed
        .into_iter()
        .filter_map(|t| {
            let path = t["Path"].as_str()?.to_string();
            let name = t["Name"].as_str()?.to_string();
            let state = t["State"].as_str().unwrap_or("Unknown").to_string();
            let key = (path.to_lowercase(), name.to_lowercase());
            let reason = catalog.get(&key).copied().unwrap_or("");
            let is_bloat = !reason.is_empty();
            let enabled = matches!(state.as_str(), "Ready" | "Running");
            Some(json!({
                "path":    path,
                "name":    name,
                "state":   state,
                "enabled": enabled,
                "isBloat": is_bloat,
                "reason":  reason,
            }))
        })
        .collect();

    let bloat_count = tasks.iter().filter(|t| t["isBloat"].as_bool().unwrap_or(false)).count();
    json!({ "tasks": tasks, "bloatCount": bloat_count })
}

pub fn toggle(path: String, name: String, enable: bool) -> Result<Value, String> {
    if !crate::ps::is_admin() {
        return Err("Scheduled Tasks ändern benötigt Adminrechte.".into());
    }
    let action = if enable { "Enable-ScheduledTask" } else { "Disable-ScheduledTask" };
    // Escape quotes in path/name
    let safe_path = path.replace('"', "'");
    let safe_name = name.replace('"', "'");
    let script = format!(
        r#"{action} -TaskPath "{safe_path}" -TaskName "{safe_name}" -ErrorAction Stop | Out-Null; "OK""#
    );
    crate::ps::run(&script)
        .map(|_| json!({ "name": name, "path": path, "enabled": enable }))
        .map_err(|e| format!("Toggle fehlgeschlagen: {e}"))
}
