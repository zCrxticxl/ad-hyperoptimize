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
                let driver_path = format!(
                    "SYSTEM\\CurrentControlSet\\Control\\Class\\{{4d36e968-e325-11ce-bfc1-08002be10318}}\\{}",
                    driver_key
                );
                if reg_read_dword("HKLM", &driver_path, name) == Some(*apply) {
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
    let driver_path = format!(
        "SYSTEM\\CurrentControlSet\\Control\\Class\\{{4d36e968-e325-11ce-bfc1-08002be10318}}\\{}",
        driver_key
    );
    match action {
        TweakAction::DriverReg { name, apply, revert } => {
            reg_write_dword("HKLM", &driver_path, name, if applying { *apply } else { *revert })
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
