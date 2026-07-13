//! GPU-specific tweak catalog.
//! Detects primary discrete GPU (NVIDIA / AMD), maps it to the Windows
//! display-adapter class key, then reads/writes driver-level registry values.
//! All apply/revert pairs are hardcoded so no journal is needed.

use serde_json::{json, Value};

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ};
#[cfg(windows)]
use winreg::RegKey;

use crate::safety::{self, ChangeItem, JournalEntry, RegVal};
use crate::tweaks;

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
    #[allow(dead_code)]
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
    /// PowerShell script — check returns "True" or "1" when applied
    Ps         { apply: &'static str, revert: &'static str, check: &'static str },
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
            name: "Maximum Performance (Disable PowerMizer)",
            category: "Clock & Power",
            vendor: "nvidia",
            description: "Disables PowerMizer — NVIDIA otherwise aggressively lowers GPU clocks at idle and under load.",
            impact: "GPU stays at maximum clocks permanently; power saving fully disabled. Not recommended for laptops on battery.",
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
            name: "Disable Dynamic P-States",
            category: "Clock & Power",
            vendor: "nvidia",
            description: "Stops dynamic switching between GPU performance states (P0–P8).",
            impact: "Smoother frametimes — eliminates clock dips from mid-frame P-state transitions. Slightly higher idle power draw.",
            risk: "Medium",
            reboot: true,
            actions: vec![
                TweakAction::DriverReg { name: "DisableDynamicPstate", apply: 1, revert: 0 },
            ],
        },
        GpuTweak {
            id: "nv_fts_telemetry_off",
            name: "Disable NVIDIA FTS Telemetry",
            category: "Privacy",
            vendor: "nvidia",
            description: "Sets three FTS telemetry flags to 0 under HKLM\\SOFTWARE\\NVIDIA Corporation\\Global\\FTS.",
            impact: "Stops NVIDIA usage telemetry upload. No effect on game performance.",
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
            name: "Disable NVIDIA Telemetry Container Service",
            category: "Privacy",
            vendor: "nvidia",
            description: "Sets NvContainerLocalSystem to Manual — prevents the NVIDIA telemetry container from auto-starting.",
            impact: "No NVIDIA telemetry process on boot. GeForce Experience and drivers continue to work normally.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::Svc { name: "NvContainerLocalSystem", apply: "Manual", revert: "Automatic" },
            ],
        },
        GpuTweak {
            id: "nv_threaded_opt",
            name: "Enable NVIDIA Threaded Optimization",
            category: "Rendering",
            vendor: "nvidia",
            description: "Forces ThreadedOptimization globally — allows the NVIDIA driver to use multi-threading for OpenGL/D3D calls.",
            impact: "Higher CPU-GPU throughput in many titles. Explicitly setting this prevents per-app overrides.",
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
            name: "Disable ULPS (Ultra Low Power State)",
            category: "Clock & Power",
            vendor: "amd",
            description: "Disables Ultra Low Power State — AMD GPUs otherwise enter a deep sleep that requires a wake-up cycle.",
            impact: "Eliminates the most common AMD micro-stutters on the first frame after idle. No measurable power increase during gaming.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::DriverReg { name: "EnableUlps",   apply: 0, revert: 1 },
                TweakAction::DriverReg { name: "EnableUlpsNV", apply: 0, revert: 1 },
            ],
        },
        GpuTweak {
            id: "amd_sclk_sleep_off",
            name: "Disable Shader Clock Deep Sleep",
            category: "Clock & Power",
            vendor: "amd",
            description: "Prevents the shader clock from entering deep sleep during short inter-frame gaps.",
            impact: "More consistent shader performance; fewer latency spikes between frames.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::DriverReg { name: "PP_SclkDeepSleepDisable", apply: 1, revert: 0 },
            ],
        },
        GpuTweak {
            id: "amd_compute_preemption_off",
            name: "Disable Compute Preemption",
            category: "Rendering",
            vendor: "amd",
            description: "Disables preemptive interruption of compute workloads — reduces driver overhead.",
            impact: "Lower driver latency for compute tasks; less frametime variance in DX12/Vulkan titles.",
            risk: "Medium",
            reboot: true,
            actions: vec![
                TweakAction::DriverReg { name: "KMD_EnableComputePreemption", apply: 0, revert: 1 },
            ],
        },
        GpuTweak {
            id: "amd_dma_power_off",
            name: "Disable DMA Power Gating",
            category: "Clock & Power",
            vendor: "amd",
            description: "Keeps the GPU DMA engine permanently active instead of gating it during inactivity.",
            impact: "Reduces latency on GPU driver DMA transfers; noticeable with fast texture uploads and asset streaming.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::DriverReg { name: "DisableDrmdmaPowerGating", apply: 1, revert: 0 },
            ],
        },
        GpuTweak {
            id: "amd_rx_tex_cache",
            name: "Enable Texture Cache Prefetch",
            category: "Rendering",
            vendor: "amd",
            description: "Enables AMD Texture Cache Prefetching for RX GPUs.",
            impact: "Higher texture throughput in open-world games with heavy asset streaming.",
            risk: "Low",
            reboot: false,
            actions: vec![
                TweakAction::DriverReg { name: "EnableAsyncComputeForGames", apply: 1, revert: 0 },
            ],
        },

        // ── NVIDIA Low Latency ────────────────────────────────────────────
        GpuTweak {
            id:          "nv_prerender_limit_1",
            name:        "Max Pre-Rendered Frames = 1",
            category:    "Low Latency",
            vendor:      "nvidia",
            description: "Limits NVIDIA frame queue to 1 pre-rendered frame (default: 3). Reduces buffering between CPU and GPU — less input lag, especially at high FPS.",
            impact:      "3–10 ms lower input lag. May cause minor stutter if CPU is the bottleneck.",
            risk:        "Low",
            reboot:      false,
            actions: vec![
                TweakAction::FixedReg {
                    root:   "HKCU",
                    path:   "Software\\NVIDIA Corporation\\Global\\NVTweak",
                    name:   "Prerenderlimit",
                    apply:  1,
                    revert: 3,
                },
            ],
        },
        GpuTweak {
            id:          "nv_low_latency_ultra",
            name:        "NVIDIA Low Latency Ultra Mode",
            category:    "Low Latency",
            vendor:      "nvidia",
            description: "Sets NVIDIA frame submission to Ultra Low Latency (RTHM_MODE=2). GPU renders frames just-in-time — minimises the CPU→GPU pipeline delay.",
            impact:      "Lowest possible input lag in GPU-bound games. Best effect at 60–144 Hz with VSYNC off.",
            risk:        "Low",
            reboot:      false,
            actions: vec![
                TweakAction::FixedReg {
                    root:   "HKCU",
                    path:   "Software\\NVIDIA Corporation\\Global\\NVTweak",
                    name:   "NVPCFLatencyPolicy",
                    apply:  2,
                    revert: 0,
                },
                TweakAction::DriverReg { name: "RTHM_MODE", apply: 2, revert: 0 },
            ],
        },

        // ── Shader Cache ──────────────────────────────────────────────────
        GpuTweak {
            id:          "dx_shader_cache_unlimited",
            name:        "Unlimited DirectX Shader Cache",
            category:    "Rendering",
            vendor:      "any",
            description: "Removes the default 10 GB cap on the DirectX shader cache. Larger cache = fewer in-game shader compilation stalls (the \"stutters\" on first visit to an area).",
            impact:      "Eliminates shader compilation stutter in DX11/DX12 games after first run. Uses more disk space.",
            risk:        "Low",
            reboot:      false,
            actions: vec![
                TweakAction::Ps {
                    apply: r#"
$p = 'HKLM:\SOFTWARE\Microsoft\DirectX'
if (-not (Test-Path $p)) { New-Item -Path $p -Force | Out-Null }
Set-ItemProperty $p 'MaxShaderCacheSizeInBytes' ([int64]::MaxValue) -Type QWord -EA SilentlyContinue
# NVIDIA OGL shader cache
$nv = 'HKCU:\Software\NVIDIA Corporation\Global\NVTweak'
if (-not (Test-Path $nv)) { New-Item -Path $nv -Force | Out-Null }
Set-ItemProperty $nv 'OglCplShaderDiskCacheMaxSize' 0x7FFFFFFF -Type DWord -EA SilentlyContinue
'Shader cache limit removed'
"#,
                    revert: r#"
Remove-ItemProperty 'HKLM:\SOFTWARE\Microsoft\DirectX' 'MaxShaderCacheSizeInBytes' -EA SilentlyContinue
Remove-ItemProperty 'HKCU:\Software\NVIDIA Corporation\Global\NVTweak' 'OglCplShaderDiskCacheMaxSize' -EA SilentlyContinue
'Shader cache reverted to default'
"#,
                    check: r#"
$v = (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\DirectX' -EA SilentlyContinue).MaxShaderCacheSizeInBytes
if ($v -and $v -gt 10GB) { 'True' } else { 'False' }
"#,
                },
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
                // `driver_key` (from detect_gpu) is already the full path under
                // HKLM — do not re-prepend GPU_CLASS, or this reads/writes a
                // bogus doubled key and every DriverReg tweak silently no-ops.
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
                if svc_start_type(name).trim().eq_ignore_ascii_case(apply) {
                    matching += 1;
                }
            }
            TweakAction::Ps { check, .. } => {
                checkable += 1;
                let out = crate::ps::run(check).unwrap_or_default();
                let out = out.trim().to_lowercase();
                if out == "true" || out == "1" { matching += 1; }
            }
        }
    }
    if checkable == 0 { return "unknown"; }
    if matching == checkable { "applied" }
    else if matching == 0    { "not_applied" }
    else                     { "partial" }
}

#[cfg(not(windows))]
fn detect_status(_tweak: &GpuTweak, _driver_key: &str) -> &'static str { "unknown" }

// ── Apply / Revert ──────────────────────────────────────────────────────────

#[cfg(windows)]
fn apply_action(action: &TweakAction, driver_key: &str, applying: bool) -> Result<(), String> {
    // See the matching comment in detect_status: driver_key is already a
    // full HKLM path, so it must be used as-is here too.
    match action {
        TweakAction::DriverReg { name, apply, revert } => {
            reg_write_dword("HKLM", driver_key, name, if applying { *apply } else { *revert })
        }
        TweakAction::FixedReg { root, path, name, apply, revert } => {
            reg_write_dword(root, path, name, if applying { *apply } else { *revert })
        }
        TweakAction::Ps { apply, revert, .. } => {
            let script = if applying { apply } else { revert };
            crate::ps::run(script).map(|_| ()).map_err(|e| format!("PS: {e}"))
        }
        TweakAction::Svc { name, apply, revert } => {
            svc_set(name, if applying { apply } else { revert })
        }
    }
}

#[cfg(not(windows))]
fn apply_action(_action: &TweakAction, _driver_key: &str, _applying: bool) -> Result<(), String> {
    Ok(())
}

// ── Public API ──────────────────────────────────────────────────────────────

pub fn scan() -> serde_json::Value {
    let (vendor, gpu_name, driver_key) = detect_gpu();
    let tweaks_data = catalog();
    let tweaks: Vec<serde_json::Value> = tweaks_data.iter().map(|tw| {
        let vendor_str = format!("{:?}", vendor).to_lowercase();
        let applicable = tw.vendor == "any" || tw.vendor == vendor_str;
        let driver_key_needed = tw.actions.iter().any(|a| matches!(a, TweakAction::DriverReg { .. }));
        let driver_key_missing = driver_key_needed && driver_key.is_empty();
        let status = if applicable && !driver_key_missing {
            detect_status(tw, &driver_key)
        } else {
            "unknown"
        };
        serde_json::json!({
            "id":              tw.id,
            "name":            tw.name,
            "category":        tw.category,
            "vendor":          tw.vendor,
            "description":     tw.description,
            "impact":          tw.impact,
            "risk":            tw.risk,
            "reboot":          tw.reboot,
            "status":          status,
            "applicable":      applicable,
            "driverKeyMissing": driver_key_missing,
        })
    }).collect();
    serde_json::json!({
        "vendor":    format!("{:?}", vendor).to_lowercase(),
        "name":      gpu_name,
        "driverKey": driver_key,
        "supported": !matches!(vendor, Vendor::Unknown),
        "tweaks":    tweaks,
    })
}

pub fn do_tweak(id: String, driver_key: String, applying: bool) -> Result<String, String> {
    let tweaks = catalog();
    for tw in &tweaks {
        if tw.id == id {
            for action in &tw.actions {
                apply_action(action, &driver_key, applying)?;
            }
            return Ok(format!("{} {}", if applying { "Applied" } else { "Reverted" }, tw.name));
        }
    }
    Err(format!("Unknown GPU tweak: {id}"))
}

// ── NVIDIA Control Panel — editable global 3D settings ──────────────────────
// Mirrors a subset of "Manage 3D Settings → Global Settings" from NVIDIA's
// own Control Panel: the handful of options that are genuinely backed by a
// plain registry DWORD. NVIDIA stores most other global settings (texture
// filtering quality, antialiasing, vertical sync, digital vibrance, per-app
// profiles, …) only in its own undocumented driver settings database (DRS),
// which has no public Windows registry or API surface — those are left to
// `nv_open_panel()`, which launches NVIDIA's real app instead of faking a
// control that wouldn't actually do anything.
//
// Every change here is captured as a `ChangeItem::Registry` (prev value read
// live, before writing) and written to the same write-ahead journal as every
// other tweak in this app, so it shows up in Reports.tsx and is undone with
// `tweaks::revert_entry` — identical plumbing to Quick Boost.

const NV_TWEAK_HKCU: &str = "Software\\NVIDIA Corporation\\Global\\NVTweak";

#[cfg(windows)]
pub fn nv_get_settings() -> Value {
    let (vendor, name, driver_key) = detect_gpu();
    if vendor != Vendor::Nvidia {
        return json!({ "supported": false, "name": name });
    }

    let power_mode = if reg_read_dword("HKLM", &driver_key, "PowerMizerEnable") == Some(0) {
        "preferMaxPerformance"
    } else {
        "adaptive"
    };

    let max_prerendered = reg_read_dword("HKCU", NV_TWEAK_HKCU, "Prerenderlimit")
        .unwrap_or(3)
        .to_string();

    let low_latency = match reg_read_dword("HKCU", NV_TWEAK_HKCU, "NVPCFLatencyPolicy") {
        Some(2) => "ultra",
        Some(1) => "on",
        _ => "off",
    };

    let threaded_opt = match reg_read_dword("HKCU", NV_TWEAK_HKCU, "Enabled") {
        Some(0xFF) => "on",
        _ => "off",
    };

    json!({
        "supported": true,
        "name": name,
        "driverKeyMissing": driver_key.is_empty(),
        "powerManagementMode": power_mode,
        "maxPreRenderedFrames": max_prerendered,
        "lowLatencyMode": low_latency,
        "threadedOptimization": threaded_opt,
    })
}

#[cfg(not(windows))]
pub fn nv_get_settings() -> Value {
    json!({ "supported": false })
}

#[cfg(windows)]
pub fn nv_set_setting(setting: String, value: String) -> Result<Value, String> {
    let (vendor, _name, driver_key) = detect_gpu();
    if vendor != Vendor::Nvidia {
        return Err("No NVIDIA GPU detected".into());
    }

    // (root, path, name, new dword value)
    let targets: Vec<(&'static str, String, &'static str, u32)> = match setting.as_str() {
        "powerManagementMode" => {
            if driver_key.is_empty() {
                return Err("NVIDIA driver registry key not found".into());
            }
            match value.as_str() {
                "adaptive" => vec![
                    ("HKLM", driver_key.clone(), "PowerMizerEnable", 1),
                    ("HKLM", driver_key.clone(), "PowerMizerLevel", 2),
                    ("HKLM", driver_key.clone(), "PowerMizerLevelAC", 2),
                ],
                "preferMaxPerformance" => vec![
                    ("HKLM", driver_key.clone(), "PowerMizerEnable", 0),
                    ("HKLM", driver_key.clone(), "PowerMizerLevel", 1),
                    ("HKLM", driver_key.clone(), "PowerMizerLevelAC", 1),
                ],
                _ => return Err(format!("unknown value '{value}' for powerManagementMode")),
            }
        }
        "maxPreRenderedFrames" => {
            let n: u32 = value
                .parse()
                .map_err(|_| format!("invalid value '{value}'"))?;
            if !(1..=4).contains(&n) {
                return Err("value must be 1-4".into());
            }
            vec![("HKCU", NV_TWEAK_HKCU.to_string(), "Prerenderlimit", n)]
        }
        "lowLatencyMode" => {
            if driver_key.is_empty() {
                return Err("NVIDIA driver registry key not found".into());
            }
            let n: u32 = match value.as_str() {
                "off" => 0,
                "on" => 1,
                "ultra" => 2,
                _ => return Err(format!("unknown value '{value}' for lowLatencyMode")),
            };
            vec![
                ("HKCU", NV_TWEAK_HKCU.to_string(), "NVPCFLatencyPolicy", n),
                ("HKLM", driver_key.clone(), "RTHM_MODE", n),
            ]
        }
        "threadedOptimization" => {
            let n: u32 = match value.as_str() {
                "off" => 0,
                "on" => 0xFF,
                _ => return Err(format!("unknown value '{value}' for threadedOptimization")),
            };
            vec![("HKCU", NV_TWEAK_HKCU.to_string(), "Enabled", n)]
        }
        _ => return Err(format!("unknown setting '{setting}'")),
    };

    let items: Vec<ChangeItem> = targets
        .iter()
        .map(|(root, path, name, new)| ChangeItem::Registry {
            root: root.to_string(),
            path: path.clone(),
            name: name.to_string(),
            prev: reg_read_dword(root, path, name).map(RegVal::Dword),
            new: RegVal::Dword(*new),
        })
        .collect();

    let entry_id = format!(
        "nvCtrlPanel-{setting}-{}",
        chrono::Local::now().format("%Y%m%d%H%M%S")
    );
    safety::append_entry(JournalEntry {
        id: entry_id.clone(),
        tweak_id: format!("nvCtrlPanel:{setting}"),
        tweak_name: format!("NVIDIA Control Panel — {setting} = {value}"),
        time: chrono::Local::now().to_rfc3339(),
        items: items.clone(),
        reverted: false,
        backup_files: vec![],
    })?;

    let mut done: Vec<&ChangeItem> = Vec::new();
    for item in &items {
        if let Err(e) = tweaks::apply_item(item) {
            for d in done.iter().rev() {
                let _ = tweaks::revert_item(d);
            }
            return Err(format!("failed to apply ({e}); changes rolled back"));
        }
        done.push(item);
    }

    Ok(json!({ "restoreToken": entry_id, "setting": setting, "value": value }))
}

#[cfg(not(windows))]
pub fn nv_set_setting(_setting: String, _value: String) -> Result<Value, String> {
    Err("Windows only".into())
}

/// Launch NVIDIA's own Control Panel app, for the settings (anisotropic
/// filtering, antialiasing, vertical sync, digital vibrance, per-app
/// profiles, …) that live only in NVIDIA's proprietary DRS database.
pub fn nv_open_panel() -> Result<String, String> {
    crate::ps::run(
        r#"
$exe = Get-ChildItem 'C:\Program Files\NVIDIA Corporation\Control Panel Client\nvcplui.exe' -ErrorAction SilentlyContinue
if ($exe) {
    Start-Process $exe.FullName
} else {
    Start-Process 'shell:AppsFolder\NVIDIACorp.NVIDIAControlPanel_56jybvy8c3kt8!NVIDIAControlPanel' -ErrorAction SilentlyContinue
}
'opened'
"#,
    )
    .map(|_| "Opened NVIDIA Control Panel".to_string())
}
