//! Hardware profiling: detect CPU/GPU/RAM/storage, compute tier ratings,
//! and generate per-page contextual warnings for the frontend.

use crate::ps;
use serde_json::{json, Value};

pub fn hw_profile() -> Value {
    let script = r#"
$out = @{}

# ── CPU ──────────────────────────────────────────────────────────────────────
try {
    $cpu = Get-CimInstance Win32_Processor -ErrorAction Stop | Select-Object -First 1
    $out.cpu = @{
        name    = $cpu.Name.Trim() -replace '\s+', ' '
        cores   = [int]$cpu.NumberOfCores
        threads = [int]$cpu.NumberOfLogicalProcessors
        maxMhz  = [int]$cpu.MaxClockSpeed
    }
} catch { $out.cpu = @{ name='Unknown CPU'; cores=4; threads=4; maxMhz=0 } }

# ── GPU ───────────────────────────────────────────────────────────────────────
try {
    $gpus = Get-CimInstance Win32_VideoController -ErrorAction Stop |
        Where-Object { $_.Name -notmatch 'Microsoft|Basic Display|Remote Desktop|VMware' } |
        Sort-Object AdapterRAM -Descending
    $g = $gpus | Select-Object -First 1
    if ($g) {
        $vramMb = if ($g.AdapterRAM -gt 0) { [math]::Round($g.AdapterRAM / 1MB) } else { 0 }
        # nvidia-smi gives exact VRAM, prefer that
        try {
            $nvVram = & nvidia-smi --query-gpu=memory.total --format=csv,noheader,nounits 2>$null |
                Select-Object -First 1
            if ($nvVram -match '^\d+') { $vramMb = [int]$nvVram }
        } catch {}
        $out.gpu = @{
            name      = $g.Name.Trim()
            vramMb    = $vramMb
            driver    = $g.DriverVersion
            allGpus   = @($gpus | ForEach-Object { $_.Name.Trim() })
        }
    } else {
        $out.gpu = @{ name='Unknown'; vramMb=0; driver=''; allGpus=@() }
    }
} catch { $out.gpu = @{ name='Unknown'; vramMb=0; driver=''; allGpus=@() } }

# ── RAM ───────────────────────────────────────────────────────────────────────
try {
    $sticks  = @(Get-CimInstance Win32_PhysicalMemory -ErrorAction Stop)
    $totalMb = [math]::Round(($sticks | Measure-Object Capacity -Sum).Sum / 1MB)
    $speedMhz = ($sticks | Select-Object -First 1).Speed
    $out.ram = @{ totalMb=[int]$totalMb; speedMhz=[int]$speedMhz; sticks=$sticks.Count }
} catch { $out.ram = @{ totalMb=0; speedMhz=0; sticks=0 } }

# ── Storage (three-tier detection) ────────────────────────────────────────────
$hasNvme = $false; $hasSsd = $false; $hasHdd = $false

# Tier 1: MSFT_Disk (most accurate, BusType 17=NVMe, MediaType 4=SSD 3=HDD)
try {
    $msdisks = @(Get-CimInstance -Namespace 'root\Microsoft\Windows\Storage' -ClassName MSFT_Disk -ErrorAction Stop)
    foreach ($d in $msdisks) {
        if ($d.BusType -eq 17) { $hasNvme = $true; $hasSsd = $true }
        elseif ($d.MediaType -eq 4) { $hasSsd = $true }
        elseif ($d.MediaType -eq 3) { $hasHdd = $true }
    }
} catch {}

# Tier 2: Get-PhysicalDisk
if (-not $hasSsd -and -not $hasHdd) {
    try {
        $disks = @(Get-PhysicalDisk -ErrorAction Stop)
        foreach ($d in $disks) {
            if ($d.BusType -eq 'NVMe') { $hasNvme = $true; $hasSsd = $true }
            elseif ($d.MediaType -eq 'SSD') { $hasSsd = $true }
            elseif ($d.MediaType -eq 'HDD') { $hasHdd = $true }
        }
    } catch {}
}

# Tier 3: Win32_DiskDrive model name matching
if (-not $hasSsd -and -not $hasHdd) {
    try {
        $drives = @(Get-CimInstance Win32_DiskDrive -ErrorAction Stop)
        foreach ($d in $drives) {
            $m = "$($d.Model) $($d.Caption) $($d.MediaType)"
            if ($m -match 'NVMe|NVME') { $hasNvme = $true; $hasSsd = $true }
            elseif ($m -match 'SSD|Solid State') { $hasSsd = $true }
            elseif ($m -match 'HDD|Hard Disk') { $hasHdd = $true }
            else { $hasSsd = $true } # modern system default
        }
    } catch {}
}

$out.storage = @{ hasNvme=$hasNvme; hasSsd=$hasSsd; hasHdd=$hasHdd }

# ── Laptop ────────────────────────────────────────────────────────────────────
try {
    $bat = @(Get-CimInstance Win32_Battery -ErrorAction Stop)
    $out.isLaptop = $bat.Count -gt 0
} catch { $out.isLaptop = $false }

# ── Network ───────────────────────────────────────────────────────────────────
try {
    $wifi = (Get-NetAdapter -ErrorAction SilentlyContinue |
        Where-Object { $_.PhysicalMediaType -match 'Native 802' -and $_.Status -eq 'Up' }).Count -gt 0
    $out.isWifi = $wifi
} catch { $out.isWifi = $false }

$out | ConvertTo-Json -Depth 4 -Compress
"#;

    let raw = ps::run_json(script).unwrap_or_else(|e| json!({ "error": e }));
    compute_profile(raw)
}

// ─── tier + warning engine ───────────────────────────────────────────────────

fn compute_profile(mut data: Value) -> Value {
    // ── read raw fields ──────────────────────────────────────────────────────
    let cpu_name    = data["cpu"]["name"].as_str().unwrap_or("").to_lowercase();
    let cpu_cores   = data["cpu"]["cores"].as_i64().unwrap_or(4);
    let gpu_name    = data["gpu"]["name"].as_str().unwrap_or("").to_lowercase();
    let gpu_vram    = data["gpu"]["vramMb"].as_i64().unwrap_or(0);
    let ram_total   = data["ram"]["totalMb"].as_i64().unwrap_or(0);
    let is_laptop   = data["isLaptop"].as_bool().unwrap_or(false);
    let is_wifi     = data["isWifi"].as_bool().unwrap_or(false);
    let has_hdd     = data["storage"]["hasHdd"].as_bool().unwrap_or(false);
    let has_nvme    = data["storage"]["hasNvme"].as_bool().unwrap_or(false);
    let has_ssd     = data["storage"]["hasSsd"].as_bool().unwrap_or(false);

    // ── vendor flags ─────────────────────────────────────────────────────────
    let is_nvidia   = gpu_name.contains("nvidia") || gpu_name.contains("geforce") ||
                      gpu_name.contains("gtx") || gpu_name.contains("rtx") || gpu_name.contains("quadro");
    let is_amd_gpu  = gpu_name.contains("amd") || gpu_name.contains("radeon");
    let is_intel_gpu= gpu_name.contains("intel") && !gpu_name.contains("core");
    let is_arc      = gpu_name.contains("arc");
    let is_integrated = (is_intel_gpu && !is_arc) ||
                        (gpu_name.contains("vega") && !gpu_name.contains("radeon rx vega")) ||
                        gpu_vram < 512;

    // detect older architecture by name patterns
    let is_older_nvidia = is_nvidia && (
        gpu_name.contains("gtx 10") || gpu_name.contains("gtx 9") ||
        gpu_name.contains("gtx 8")  || gpu_name.contains("gtx 7") ||
        gpu_name.contains("gtx 6")  || gpu_name.contains("gtx 745") ||
        gpu_name.contains("gtx 750")
    );
    let is_older_amd = is_amd_gpu && (
        gpu_name.contains("rx 5") || gpu_name.contains("rx 4") ||
        gpu_name.contains("r9 ")  || gpu_name.contains("r7 ") ||
        gpu_name.contains("r5 ")
    );
    let is_older_arch = is_older_nvidia || is_older_amd;

    // ── CPU tier ─────────────────────────────────────────────────────────────
    let cpu_tier = if cpu_cores <= 2
        || cpu_name.contains("celeron") || cpu_name.contains("pentium")
        || cpu_name.contains("atom")    || cpu_name.contains("n3")
        || cpu_name.contains("n4")      || cpu_name.contains("n5")
    {
        "budget"
    } else if cpu_cores <= 6 {
        "mid"
    } else {
        "high"
    };

    // ── GPU tier ─────────────────────────────────────────────────────────────
    let gpu_tier = if is_integrated {
        "integrated"
    } else if gpu_vram > 0 && gpu_vram < 2048
        || gpu_name.contains("gt 710") || gpu_name.contains("gt 730")
        || gpu_name.contains("gt 1030") || gpu_name.contains("gtx 750")
    {
        "budget"
    } else if gpu_vram < 8192 || is_older_nvidia || is_older_amd {
        "mid"
    } else {
        "high"
    };

    // ── RAM tier ─────────────────────────────────────────────────────────────
    let ram_tier = if ram_total > 0 && ram_total < 8192 { "low" }
                   else if ram_total < 32768            { "ok"  }
                   else                                 { "good" };

    // ── Storage tier ─────────────────────────────────────────────────────────
    let storage_tier = if has_nvme { "nvme" }
                       else if has_ssd { "sata_ssd" }
                       else if has_hdd { "hdd" }
                       else { "unknown" };

    // ── Warnings ─────────────────────────────────────────────────────────────
    let mut warnings: Vec<Value> = Vec::new();

    macro_rules! w {
        ($page:expr, $id:expr, $sev:expr, $title:expr, $msg:expr) => {
            warnings.push(json!({
                "page":     $page,
                "id":       $id,
                "severity": $sev,
                "title":    $title,
                "message":  $msg,
            }));
        };
    }

    // ─── GPU Tweaks ───────────────────────────────────────────────────────────
    if is_integrated && !is_arc {
        w!("gpu_tweaks", "integrated_only", "danger",
            "No Discrete GPU Detected",
            "Most GPU tweaks require a dedicated graphics card. Only service-level tweaks (HAGS, MPO) may apply to integrated graphics.");
    }
    if gpu_tier == "budget" && !is_integrated {
        w!("gpu_tweaks", "budget_gpu", "warning",
            "Budget GPU",
            "Budget GPUs are more sensitive to driver-level tweaks. Apply changes one at a time and test stability before applying more.");
    }
    if is_older_arch {
        w!("gpu_tweaks", "older_arch_thermal", "warning",
            "Older GPU Architecture — Thermal Risk",
            "GTX 900/1000 and RX 400/500 series GPUs can run hot under P-state locks or forced max clocks. Monitor temps closely and ensure airflow is good.");
    }
    if gpu_vram > 0 && gpu_vram < 4096 && !is_integrated {
        w!("gpu_tweaks", "low_vram", "info",
            "Low VRAM (<4 GB)",
            "No tweak increases VRAM capacity. In modern games, low VRAM causes stuttering even with optimal driver settings. Lower in-game texture quality.");
    }
    if is_laptop && (is_nvidia || is_amd_gpu) {
        w!("gpu_tweaks", "laptop_power_limit", "info",
            "Laptop GPU — Power Limited",
            "Laptop GPUs are throttled by TDP limits set by the manufacturer. P-state and frequency tweaks may have limited effect and can increase heat.");
    }

    // ─── PerfTweaks ──────────────────────────────────────────────────────────
    if cpu_cores <= 2 {
        w!("perf_tweaks", "very_low_cores", "warning",
            "Very Low Core Count (≤2 Cores)",
            "Timer resolution improvements have diminishing returns on 2-core CPUs. Prioritize RAM and storage upgrades for the biggest gains.");
    }
    if is_laptop {
        w!("perf_tweaks", "laptop_timer", "info",
            "Laptop — Battery Impact",
            "High-resolution timers (0.5 ms) significantly increase power consumption. Use only while plugged in for gaming sessions.");
    }
    if ram_total > 0 && ram_total < 8192 {
        w!("perf_tweaks", "low_ram_pagefile", "warning",
            "Low RAM (<8 GB) — Keep Pagefile On",
            "With less than 8 GB of RAM, disabling the pagefile risks crashes and out-of-memory errors. Recommended: set pagefile to System Managed.");
    }
    if is_wifi {
        w!("perf_tweaks", "wifi_network", "info",
            "Wi-Fi Connection Detected",
            "Network tweaks are optimized for wired Ethernet. On Wi-Fi, adapter queue and interrupt tweaks may have less effect — a wired connection is strongly recommended for gaming.");
    }

    // ─── GameBooster ─────────────────────────────────────────────────────────
    if ram_total > 0 && ram_total < 8192 {
        w!("game_booster", "low_ram", "warning",
            "Critical: Low RAM (<8 GB)",
            "Every freed megabyte matters. Kill all non-essential background processes before launching your game. Consider a RAM upgrade for lasting improvement.");
    }
    if is_integrated && !is_arc {
        w!("game_booster", "no_discrete_gpu", "warning",
            "No Discrete GPU",
            "Game Booster's GPU Performance Mode requires a dedicated GPU. CPU and process priority optimizations still apply.");
    }
    if cpu_tier == "budget" {
        w!("game_booster", "budget_cpu_priority", "info",
            "Budget CPU — Priority Boost Helps",
            "Setting your game to High priority has a larger impact on low core count CPUs. Make sure to kill all background apps first.");
    }
    if is_laptop {
        w!("game_booster", "laptop_perf", "info",
            "Laptop — Plug In Before Boosting",
            "Ensure your laptop is plugged into power before boosting. Battery saver modes will cap CPU and GPU performance regardless of boost settings.");
    }

    // ─── PowerPlan ───────────────────────────────────────────────────────────
    if is_laptop {
        w!("power_plan", "laptop_battery_drain", "warning",
            "Laptop — High Performance Drains Battery",
            "High Performance and Ultimate Performance plans disable CPU frequency scaling. This can reduce battery life by 30–60%. Use only while plugged in.");
    }
    if cpu_tier == "budget" {
        w!("power_plan", "budget_cpu_plan", "info",
            "Budget CPU — Power Plan is High Impact",
            "Low-end CPUs downclock aggressively on Balanced plan. Switching to High Performance ensures sustained base clocks — high impact for budget hardware.");
    }
    if !is_laptop && gpu_tier != "high" {
        w!("power_plan", "desktop_custom_plan_thermal_stack", "warning",
            "Custom/Third-Party Power Plans Can Stack Heat With GPU Tweaks",
            "Plans created by tools like Winhance unlock hidden Advanced Power Settings (Processor Boost Mode, Core Parking, min processor state) that pin the CPU at max clock 24/7 — the same thing 'Maximum Performance'/'Disable Dynamic P-States' GPU tweaks do for the GPU. On a card below high-tier in an OEM/compact case, running both chips pinned at max simultaneously raises shared case temps and can push the GPU into thermal throttling sooner, lowering FPS instead of raising it. If you're using a custom power plan AND GPU clock-lock tweaks together and saw an FPS regression, test switching back to Balanced first.");
    }

    // ─── Dashboard ───────────────────────────────────────────────────────────
    if has_hdd && !has_ssd {
        w!("dashboard", "hdd_only", "warning",
            "No SSD Detected",
            "An SSD provides 5–10× faster Windows boot and load times. Upgrading to an SSD is the single highest-impact hardware improvement for most systems.");
    }
    if ram_total > 0 && ram_total < 8192 {
        w!("dashboard", "low_ram", "warning",
            "Low RAM (<8 GB)",
            "16 GB is the current gaming standard. With <8 GB, games regularly stutter due to RAM pressure. Optimizations help but won't overcome this hardware limit.");
    }
    if is_integrated && !is_arc {
        w!("dashboard", "no_discrete_gpu", "info",
            "Integrated Graphics Only",
            "A discrete GPU unlocks the full benefit of GPU tweaks and enables significantly higher in-game performance.");
    }
    if is_wifi {
        w!("dashboard", "wifi_latency", "info",
            "Wi-Fi Connection",
            "Wired Ethernet provides lower ping, less jitter, and more consistent speeds for gaming. Consider switching if latency matters.");
    }

    // ─── Debloater ───────────────────────────────────────────────────────────
    if ram_total > 0 && ram_total < 16384 {
        w!("debloater", "low_ram_debloat", "info",
            "RAM Below 16 GB — Debloating Recommended",
            "Removing bloatware frees background RAM and reduces idle CPU usage. High impact on systems with 8 GB or less.");
    }

    // ─── Latency ─────────────────────────────────────────────────────────────
    if is_older_nvidia {
        w!("latency", "older_nvidia_dpc", "info",
            "Older NVIDIA GPU — Check Driver Version",
            "GTX 900/1000-series GPUs can generate elevated DPC latency with outdated drivers. Ensure you're on the latest Game Ready Driver.");
    }
    if is_wifi {
        w!("latency", "wifi_dpc", "warning",
            "Wi-Fi — High DPC Latency Risk",
            "Wi-Fi drivers are a common source of DPC latency spikes. If stalls appear in the Latency Analyzer, try disabling Wi-Fi power saving in Device Manager.");
    }

    // ─── Per-tweak hardware risk engine ──────────────────────────────────────
    // Maps specific tweak IDs (from tweaks.rs + gputweaks.rs catalogs) to a
    // hardware-aware risk verdict. Severity "danger" means: this exact tweak
    // is a known cause of REGRESSION (lower FPS / instability) on this exact
    // hardware class — frontend must force an explicit "apply anyway" step.
    // "warning" = situational, show badge + let normal confirm flow handle it.
    let mut tweak_risks = serde_json::Map::new();

    macro_rules! risk {
        ($id:expr, $sev:expr, $title:expr, $msg:expr) => {
            tweak_risks.insert($id.to_string(), json!({
                "severity": $sev, "title": $title, "message": $msg,
            }));
        };
    }

    let gpu_not_high = gpu_tier != "high"; // budget | mid | integrated

    // ── GPU clock/power locks — the #1 real-world regression cause ─────────
    // Forcing max clock 24/7 only helps if the cooler can sustain it. On any
    // GPU below "high" tier (most laptop GPUs, GTX 16-series and below,
    // OEM/SFF builds) this raises baseline temp until the card hits its
    // thermal limit and boost-clocks DROP below what adaptive boosting
    // would have given — net result: fewer FPS than stock.
    if is_laptop && (is_nvidia || is_amd_gpu) {
        // Laptop-specific message takes priority over the generic desktop
        // thermal-throttle one below — laptop GPUs are TDP-capped, which is
        // the more relevant/specific explanation for this hardware class.
        risk!("nv_power_max_perf", "danger",
            "Laptop GPU — Power/Thermal Limited",
            "Laptop GPUs are capped by a manufacturer TDP limit, not just clock speed. Forcing max clock cannot exceed that cap — it only adds heat, which throttles the CPU/GPU pair via shared cooling. Expect no FPS gain and possible loss.");
    } else if is_nvidia && gpu_not_high {
        risk!("nv_power_max_perf", "danger",
            "Thermal Throttle Risk on This GPU",
            "Locks your GPU at maximum clock permanently. On this card/cooling combo this usually raises baseline temperature until the GPU hits its thermal limit mid-game and throttles BELOW stock boost clocks — net result is often lower FPS, not higher. Only beneficial on high-end cards with strong cooling (large air/AIO). Test with a benchmark before/after; revert if FPS drops or temps exceed ~80°C.");
    }
    if is_nvidia && gpu_not_high {
        risk!("nv_dynamic_pstate_off", "danger",
            "Thermal Throttle Risk on This GPU",
            "Functionally overlaps with 'Maximum Performance' — both force the GPU off its adaptive power curve. Combining them (or even just one, on this GPU tier) is the most common cause of FPS regressions reported on mid/budget cards. Apply only one at a time, monitor temps, revert if FPS drops.");
    }

    // ── Hardware-accelerated GPU scheduling ─────────────────────────────────
    if is_older_arch {
        risk!("hags_on", "warning",
            "HAGS on Older GPU Architecture",
            "Hardware-Accelerated GPU Scheduling was built for newer schedulers (NVIDIA 10-series+/AMD RDNA+ with current drivers) but real-world stutter/regression reports cluster on older architectures and outdated drivers. Benchmark before/after — revert if frametimes get worse.");
        risk!("amd_compute_preemption_off", "warning",
            "Older AMD Architecture",
            "Disabling compute preemption changes driver scheduling behavior that varies a lot by GPU generation. On older RX 400/500-series cards this can increase stutter instead of reducing it. Test before/after.");
    }

    // ── CPU scheduler tweaks — risk on low core counts ──────────────────────
    if cpu_tier == "budget" {
        risk!("priority_separation", "warning",
            "Aggressive Scheduler Bias on Low Core Count",
            "Win32PrioritySeparation 0x26 gives the foreground app a hard 3:1 quanta bias. On 2-core/4-thread CPUs this can starve background services (audio, anti-cheat helpers, drivers) instead of helping — the opposite of the intended effect. Watch for audio crackle or input stutter after applying.");
        risk!("net_offload_disable", "warning",
            "Budget CPU — Offload Disabling Increases CPU Load",
            "Disabling NIC offloads (TCP checksum, LSO, RSS) moves that work from the network card back onto the CPU. On a low core-count CPU under load (game + voice chat + overlay) this can add enough overhead to cost more frame time than the latency it saves. Test before/after with a frame-time graph, not just ping.");
        risk!("mmcss_games_task", "warning",
            "Maximum MMCSS Priority on Low Core Count",
            "GPU Priority 8 + CPU Priority 6 increases scheduling priority for any MMCSS-registered process. On budget CPUs (≤4 threads) this can cause contention with system threads and reduce, not improve, frame consistency.");
    }

    // ── Power throttling / heat & battery ───────────────────────────────────
    if is_laptop {
        risk!("power_throttling_off", "warning",
            "Laptop — Heat & Battery Impact",
            "Disabling EcoQoS power throttling stops Windows from parking background processes onto efficiency cores. On a laptop this raises sustained power draw and heat, which can trigger CPU thermal throttling under combined CPU+GPU load — possibly netting lower sustained FPS than with throttling enabled. Use only plugged in, monitor CPU temps.");
    }

    // ── Low RAM — pagefile / RAM-dependent tweaks ───────────────────────────
    if ram_tier == "low" {
        risk!("pagefile_disable", "danger",
            "Low RAM — Disabling Pagefile Risks Crashes",
            "With under 8 GB RAM, the pagefile is not optional headroom — it's actively used. Disabling it on this system risks out-of-memory crashes and application instability. Keep at minimum 'System Managed'.");
        risk!("disable_paging_executive", "danger",
            "Low RAM — Keep Kernel in RAM Not Recommended",
            "This tweak's own rationale only applies to systems with ≥8 GB RAM. On this system it pins kernel/driver pages permanently in RAM, removing headroom you don't have — increases the chance of paging thrash and crashes instead of reducing latency.");
    }

    // ── SysMain/Superfetch — helps HDDs, hurts nothing on SSD but is a no-op ─
    // The tweak's own catalog name says "(SSD systems)" — disabling it on an
    // HDD-only system removes the prefetching that masks slow seek times.
    if has_hdd && !has_ssd {
        risk!("sysmain_off", "danger",
            "HDD Detected — SysMain/Superfetch Helps You",
            "This tweak is intended for SSD systems. Your system drive is a mechanical HDD, where SysMain's prefetching actively hides slow seek/access times. Disabling it on an HDD-only system will make app launches and boot feel slower, not faster.");
    }

    // ── Laptop power-draw tweaks — heat & battery, not 'free' performance ───
    if is_laptop {
        risk!("core_parking_off", "warning",
            "Laptop — Battery & Heat Impact",
            "Keeps every CPU core powered at all times instead of parking idle cores. On a laptop this raises sustained power draw and heat — can shorten battery life noticeably and, under combined CPU+GPU load, contribute to thermal throttling that cancels out the latency gain this tweak aims for.");
        risk!("usb_suspend_off", "warning",
            "Laptop — Battery Impact",
            "Keeps all USB devices fully powered, never suspended. Fine on AC power; on battery this is a measurable, continuous drain for the questionable benefit of avoiding mouse/headset micro-stutter most users never notice.");
        risk!("timer_resolution_min", "warning",
            "Laptop — Battery & Heat Impact",
            "Forcing 0.5ms timer resolution wakes the CPU far more often than the Windows default (~15.6ms), increasing power draw and heat. On a laptop this measurably shortens battery life and can add to thermal load under sustained gaming.");
        risk!("power_plan_ultimate", "warning",
            "Laptop — Ultimate Performance Disables Power Saving",
            "Ultimate Performance disables nearly all power-saving parking/throttling. On a laptop this raises idle power draw and heat noticeably and shortens battery life — only use it on AC power.");
    }

    // ── Unknown/third-party power plans (Winhance, OEM-bundled, custom) ────
    // Not hardware-conditional — flagged unconditionally because we cannot
    // see what Advanced Power Settings the plan actually contains. Always
    // surfaced so the user knows to check before relying on "good FPS" from
    // a plan whose settings weren't reviewed.
    risk!("power_plan_custom_unknown", "warning",
        "Custom/Unknown Power Plan",
        "This plan wasn't created by Windows or this app — its Advanced Power Settings (min processor state, Core Parking, Boost Mode) are unknown and may already be forcing max CPU clock 24/7. Combined with GPU clock-lock tweaks this can cause unexpected FPS regressions. Check Power Options → Change plan settings → Change advanced power settings before trusting it for gaming.");

    // ── attach computed fields ────────────────────────────────────────────────
    data["cpu"]["tier"]        = json!(cpu_tier);
    data["gpu"]["tier"]        = json!(gpu_tier);
    data["gpu"]["isIntegrated"]= json!(is_integrated);
    data["gpu"]["isOlderArch"] = json!(is_older_arch);
    data["ram"]["tier"]        = json!(ram_tier);
    data["storage"]["tier"]    = json!(storage_tier);
    data["warnings"]           = json!(warnings);
    data["tweakRisks"]         = Value::Object(tweak_risks);

    data
}
