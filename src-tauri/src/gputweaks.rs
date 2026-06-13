//! GPU-specific tweak catalog.
//! Detects primary discrete GPU (NVIDIA / AMD), maps it to the Windows
//! display-adapter class key, then reads/writes driver-level registry values.
//! All apply/revert pairs are hardcoded so no journal is needed.

use serde_json::{json, Value};

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
#[cfg(windows)]
use winreg::RegKey;

const GPU_CLASS: &str =
    "SYSTEM\\CurrentControlSet\\Control\\Class\\{4d36e968-e325-11ce-bfc1-08002be10318}";

// ── registry helpers ────────────────────────────────────────────────────────

#[cfg(windows)]
fn hive(root: &str) -> RegKey {
    RegKey::predef(if root == "HKLM" { HKEY_LOCAL_MACHINE } else { HKEY_CURRENT_USER })
}

#[cfg(windows)]
fn reg_read_dword(root: &str, path: &str, name: &str) -> Option<u32> {
    hive(root).open_subkey_with_flags(path, KEY_READ).ok()?.get_value(name).ok()
}

#[cfg(windows)]
fn reg_write_dword(root: &str, path: &str, name: &str, v: u32) -> Result<(), String> {
    let (key, _) = hive(root)
        .create_subkey(path)
        .map_err(|e| format!("open {path}: {e}"))?;
    key.set_value(name, &v).map_err(|e| format!("set {name}: {e}"))
}

fn svc_start_type(name: &str) -> String {
    crate::ps::run(&format!(
        "(Get-Service -Name '{name}' -ErrorAction SilentlyContinue).StartType"
    ))
    .unwrap_or_default()
    .trim()
    .to_string()
}

fn svc_set(name: &str, mode: &str) -> Result<(), String> {
    crate::ps::run(&format!(
        "Set-Service -Name '{name}' -StartupType {mode} -ErrorAction Stop; 'OK'"
    ))
    .map(|_| ())
}

// ── GPU detection ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Vendor { Nvidia, Amd, Intel, Unknown }

impl Vendor {
    fn as_str(&self) -> &'static str {
        match self { Self::Nvidia => "nvidia", Self::Amd => "amd", Self::Intel => "intel", Self::Unknown => "unknown" }
    }
    fn from_name(name: &str) -> Self {
        let n = name.to_lowercase();
        if n.contains("nvidia") || n.contains("geforce") || n.contains("quadro") { Self::Nvidia }
        else if n.contains("amd") || n.contains("radeon") { Self::Amd }
        else if n.contains("intel") && (n.contains("arc") || n.contains("iris") || n.contains("uhd") || n.contains("hd graphics")) { Self::Intel }
        else { Self::Unknown }
    }
}

fn detect_gpu() -> (Vendor, String, String) {
    // Returns (Vendor, display name, driver-class subkey path under HKLM)
    let script = r#"
$base = "HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}"
$all = @()
Get-ChildItem $base -ErrorAction SilentlyContinue | ForEach-Object {
    $p = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
    if ($p -and $p.DriverDesc -and $p.DriverDesc.Trim() -ne '') {
        $all += [PSCustomObject]@{ Index=$_.PSChildName; Name=$p.DriverDesc.Trim() }
    }
}
$gpu = $all | Where-Object { $_.Name -match 'NVIDIA|GeForce|Quadro' } | Select-Object -First 1
if (-not $gpu) { $gpu = $all | Where-Object { $_.Name -match 'AMD|Radeon' } | Select-Object -First 1 }
if (-not $gpu) { $gpu = $all | Where-Object { $_.Name -match 'Intel.*Arc' } | Select-Object -First 1 }
if (-not $gpu) { $gpu = $all | Where-Object { $_.Name -notmatch 'Microsoft|Basic Display|Remote Desktop' } | Select-Object -First 1 }
if ($gpu) { $gpu | ConvertTo-Json -Compress } else { '{"Index":"","Name":"Unknown"}' }
"#;

    let raw = crate::ps::run(script).unwrap_or_default();
    let parsed: Value = serde_json::from_str(raw.trim()).unwrap_or(json!({}));

    let name = parsed["Name"].as_str().unwrap_or("Unknown").to_string();
    let index = parsed["Index"].as_str().unwrap_or("").to_string();
    let vendor = Vendor::from_name(&name);
    let driver_key = if index.is_empty() {
        String::new()
    } else {
        format!("{GPU_CLASS}\\{index}")
    };
    (vendor, name, driver_key)
}

// ── Tweak definitions ───────────────────────────────────────────────────────

#[derive(Clone)]
enum TweakAction {
    /// Value in the detected driver-class key (HKLM\...\{index})
    DriverReg { name: &'static str, apply: u32, revert: u32 },
    /// Fixed HKLM or HKCU path
    FixedReg   { root: &'static str, path: &'static str, name: &'static str, apply: u32, revert: u32 },
    /// Windows service startup type
    Svc        { name: &'static str, apply: &'static str, revert: &'static str },
}

struct GpuTweak {
    id:          &'static str,
    name:        &'static str,
    category:    &'static str,
    vendor:      &'static str, // "nvidia" | "amd" | "any"
    description: &'static str,
    impact:      &'static str,
    risk:        &'static str,
    reboot:      bool,
    actions:     Vec<TweakAction>,
}

fn catalog() -> Vec<GpuTweak> {
    vec![
        // ── NVIDIA ────────────────────────────────────────────────────────
        GpuTweak {
            id: "nv_power_max_perf",
            name: "Maximum Performance (PowerMizer aus)",
            category: "Takt & Power",
            vendor: "nvidia",
            description: "Deaktiviert PowerMizer — NVIDIA senkt sonst im Leerlauf und unter Last die GPU-Takte aggressiv.",
            impact: "GPU bleibt dauerhaft auf maximalen Taktraten; Stromsparfunktion komplett deaktiviert. Nicht für Laptops im Akkubetrieb.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::DriverReg { name: "PowerMizerEnable",  apply: 0, revert: 1 },
                TweakAction::DriverReg { name: "PowerMizerLevel",   apply: 1, revert: 0 },
                TweakAction::DriverReg { name: "PowerMizerLevelAC", apply: 1, revert: 0 },
            ],
        },
        GpuTweak {
            id: "nv_dynamic_pstate_off",
            name: "Dynamic P-States deaktivieren",
            category: "Takt & Power",
            vendor: "nvidia",
            description: "Stoppt das dynamische Wechseln zwischen GPU-Performance-States (P0–P8).",
            impact: "Gleichmäßigere Frametimes — kein Einbruch durch P-State-Wechsel mid-Frame. Etwas höherer Idle-Verbrauch.",
            risk: "Medium",
            reboot: true,
            actions: vec![
                TweakAction::DriverReg { name: "DisableDynamicPstate", apply: 1, revert: 0 },
            ],
        },
        GpuTweak {
            id: "nv_fts_telemetry_off",
            name: "NVIDIA FTS-Telemetrie deaktivieren",
            category: "Datenschutz",
            vendor: "nvidia",
            description: "Setzt drei FTS-Telemetrie-Flags auf 0 in HKLM\\SOFTWARE\\NVIDIA Corporation\\Global\\FTS.",
            impact: "Kein Senden von NVIDIA-Nutzungstelemetrie. Kein Einfluss auf Spieleleistung.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::FixedReg { root:"HKLM", path:"SOFTWARE\\NVIDIA Corporation\\Global\\FTS", name:"EnableRID44231", apply:0, revert:1 },
                TweakAction::FixedReg { root:"HKLM", path:"SOFTWARE\\NVIDIA Corporation\\Global\\FTS", name:"EnableRID64640", apply:0, revert:1 },
                TweakAction::FixedReg { root:"HKLM", path:"SOFTWARE\\NVIDIA Corporation\\Global\\FTS", name:"EnableRID66610", apply:0, revert:1 },
            ],
        },
        GpuTweak {
            id: "nv_telemetry_svc_off",
            name: "NVIDIA Container Telemetrie-Dienst deaktivieren",
            category: "Datenschutz",
            vendor: "nvidia",
            description: "Setzt NvContainerLocalSystem auf Manual — hält NVIDIA Telemetry-Container-Prozess vom Autostart ab.",
            impact: "Kein NVIDIA-Telemetrie-Prozess beim Booten. GeForce Experience und Treiber funktionieren weiterhin.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::Svc { name: "NvContainerLocalSystem", apply: "Manual", revert: "Automatic" },
            ],
        },
        GpuTweak {
            id: "nv_threaded_opt",
            name: "NVIDIA Thread-Optimierung aktivieren",
            category: "Rendering",
            vendor: "nvidia",
            description: "Setzt ThreadedOptimization global auf 1 — erlaubt NVIDIA Treiber Multi-Threading für OpenGL/D3D-Calls.",
            impact: "Höherer CPU-GPU-Durchsatz in vielen Titeln. Standard sollte bereits aktiv sein; explizit setzen verhindert per-App-Override.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::FixedReg {
                    root: "HKCU",
                    path: "SOFTWARE\\NVIDIA Corporation\\Global\\NVTweak",
                    name: "Enabled",
                    apply: 0xFF,
                    revert: 0,
                },
            ],
        },

        // ── AMD ───────────────────────────────────────────────────────────
        GpuTweak {
            id: "amd_ulps_off",
            name: "ULPS (Ultra Low Power State) deaktivieren",
            category: "Takt & Power",
            vendor: "amd",
            description: "Deaktiviert Ultra Low Power State — AMD-GPUs fallen sonst in einen Tiefschlaf-Zustand, aus dem sie erst wieder aufgeweckt werden müssen.",
            impact: "Eliminiert die häufigsten AMD-Mikro-Ruckler beim ersten Frame nach einem Leerlauf. Kein messbarer Mehrverbrauch bei Gaming.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::DriverReg { name: "EnableUlps",   apply: 0, revert: 1 },
                TweakAction::DriverReg { name: "EnableUlpsNV", apply: 0, revert: 1 },
            ],
        },
        GpuTweak {
            id: "amd_sclk_sleep_off",
            name: "Shader-Takt Deep Sleep deaktivieren",
            category: "Takt & Power",
            vendor: "amd",
            description: "Verhindert, dass der Shader-Takt während kurzer Lücken zwischen Frames in Deep Sleep fällt.",
            impact: "Gleichmäßigere Shader-Performance; weniger Latenz-Spitzen zwischen Frames.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::DriverReg { name: "PP_SclkDeepSleepDisable", apply: 1, revert: 0 },
            ],
        },
        GpuTweak {
            id: "amd_compute_preemption_off",
            name: "Compute-Preemption deaktivieren",
            category: "Rendering",
            vendor: "amd",
            description: "Deaktiviert das unterbrechende Preemption von Compute-Workloads — reduziert Treiber-Overhead.",
            impact: "Geringere Treiber-Latenz bei Compute-Aufgaben; weniger Frametime-Varianz in DX12/Vulkan-Spielen.",
            risk: "Medium",
            reboot: true,
            actions: vec![
                TweakAction::DriverReg { name: "KMD_EnableComputePreemption", apply: 0, revert: 1 },
            ],
        },
        GpuTweak {
            id: "amd_dma_power_off",
            name: "DMA Power Gating deaktivieren",
            category: "Takt & Power",
            vendor: "amd",
            description: "Hält den DMA-Engine der GPU dauerhaft aktiv statt ihn bei Inaktivität abzuschalten.",
            impact: "Reduziert Latenz beim GPU-Treiber-DMA-Transfer; merkbar bei schnellen Textur-Uploads und Asset-Streaming.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::DriverReg { name: "DisableDrmdmaPowerGating", apply: 1, revert: 0 },
            ],
        },
        GpuTweak {
            id: "amd_rx_tex_cache",
            name: "Texture-Cache-Prefetch aktivieren",
            category: "Rendering",
            vendor: "amd",
            description: "Aktiviert AMD Texture Cache Prefetching für RX-GPUs.",
            impact: "Höherer Textur-Durchsatz in Open-World-Spielen mit vielen Asset-Streams.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::DriverReg { name: "EnableAsyncComputeForGames", apply: 1, revert: 0 },
            ],
        },
    ]
}

// ── Status detection ────────────────────────────────────────────────────────

#[cfg(windows)]
fn detect_status(tweak: &GpuTweak, driver_key: &str) -> &'static str {
    let mut checkable = 0usize;
    let mut matching  = 0usize;

    for action in &tweak.actions {
        match action {
            TweakAction::DriverReg { name, apply, .. } => {
                checkable += 1;
                if driver_key.is_empty() { continue; }
                if reg_read_dword("HKLM", driver_key, name) == Some(*apply) {
                    matching += 1;
                }
            }
            TweakAction::FixedReg { root, path, name, apply, .. } => {
                checkable += 1;
                if reg_read_dword(root, path, name) == Some(*apply) {
                    matching += 1;
                }
            }
            TweakAction::Svc { name, apply, .. } => {
                checkable += 1;
                if svc_start_type(name).eq_ignore_ascii_case(apply) {
                    matching += 1;
                }
            }
        }
    }

    if checkable == 0 { return "unknown"; }
    if matching == checkable { return "applied"; }
    if matching > 0 { return "partial"; }
    "not_applied"
}

#[cfg(not(windows))]
fn detect_status(_: &GpuTweak, _: &str) -> &'static str { "unknown" }

// ── Public API ──────────────────────────────────────────────────────────────

pub fn scan() -> Value {
    let (vendor, name, driver_key) = detect_gpu();

    let applicable_tweaks: Vec<Value> = catalog()
        .iter()
        .filter(|t| t.vendor == vendor.as_str() || t.vendor == "any")
        .map(|t| {
            let status = detect_status(t, &driver_key);
            let needs_driver_key = t.actions.iter().any(|a| matches!(a, TweakAction::DriverReg { .. }));
            json!({
                "id":          t.id,
                "name":        t.name,
                "category":    t.category,
                "vendor":      t.vendor,
                "description": t.description,
                "impact":      t.impact,
                "risk":        t.risk,
                "reboot":      t.reboot,
                "status":      status,
                "needsDriverKey": needs_driver_key,
                "driverKeyMissing": needs_driver_key && driver_key.is_empty(),
            })
        })
        .collect();

    json!({
        "vendor":    vendor.as_str(),
        "name":      name,
        "driverKey": driver_key,
        "tweaks":    applicable_tweaks,
        "supported": vendor != Vendor::Unknown,
    })
}

pub fn apply_tweak(id: String, driver_key: String) -> Result<Value, String> {
    do_tweak(&id, &driver_key, true)
}

pub fn revert_tweak(id: String, driver_key: String) -> Result<Value, String> {
    do_tweak(&id, &driver_key, false)
}

fn do_tweak(id: &str, driver_key: &str, applying: bool) -> Result<Value, String> {
    let tweak = catalog()
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("unbekannter Tweak: {id}"))?;

    for action in &tweak.actions {
        #[cfg(windows)]
        match action {
            TweakAction::DriverReg { name, apply, revert } => {
                if driver_key.is_empty() {
                    return Err("GPU-Treiber-Key nicht gefunden — GPU nicht unterstützt oder nicht erkannt.".into());
                }
                let val = if applying { *apply } else { *revert };
                reg_write_dword("HKLM", driver_key, name, val)
                    .map_err(|e| format!("DriverReg {name}: {e}"))?;
            }
            TweakAction::FixedReg { root, path, name, apply, revert } => {
                let val = if applying { *apply } else { *revert };
                reg_write_dword(root, path, name, val)
                    .map_err(|e| format!("FixedReg {name}: {e}"))?;
            }
            TweakAction::Svc { name, apply, revert } => {
                let mode = if applying { apply } else { revert };
                svc_set(name, mode).map_err(|e| format!("Svc {name}: {e}"))?;
            }
        }
        #[cfg(not(windows))]
        let _ = (action, applying, driver_key);
    }

    Ok(json!({ "id": id, "status": if applying { "applied" } else { "reverted" } }))
}
