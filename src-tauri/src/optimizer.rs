//! System Health Check and verified optimization engine.
//!
//! Detect → analyse → diagnose → recommend → (user-approved) apply → verify.
//! Every decision is made by pure functions over detected facts and live tweak
//! states (unit-tested below); the I/O layer only gathers those inputs and
//! executes approved tweaks through the journaled tweak engine.
//!
//! Nothing here claims a measured performance gain. "Verified" means the
//! changed setting was read back from Windows after the change.

use crate::optregistry::{self, Evidence, Facts, OptMeta, Reboot};
use crate::safety::JournalEntry;
use serde_json::{json, Value};

// ── Detected system facts ────────────────────────────────────────────────────

const PLAN_HIGH_PERFORMANCE: &str = "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c";
const PLAN_POWER_SAVER: &str = "a1841308-3541-4fab-bc81-f71556f20b4a";

#[derive(Clone, Debug, Default)]
pub struct System {
    pub build: u32,
    pub ubr: u32,
    pub display_version: String,
    pub edition: String,
    pub cpu_name: String,
    pub cpu_cores: u32,
    pub cpu_threads: u32,
    pub gpus: Vec<(String, String, String)>, // (name, driver version, driver date yyyy-mm-dd)
    pub ram_mb: u64,
    pub has_battery: bool,
    pub has_hdd: bool,
    pub system_disk_hdd: bool,
    pub system_free_pct: Option<f64>,
    pub active_plan: Option<String>,
    pub plans: Vec<String>,
    pub ultimate_active: bool,
    pub pagefile_disabled: bool,
    pub defender_realtime: Option<bool>,
}

/// Locale-independent model-name rules (documented in the UI as such).
pub fn is_dual_ccd_x3d(cpu: &str) -> bool {
    let c = cpu.to_ascii_uppercase();
    ["7950X3D", "7900X3D", "9950X3D", "9900X3D"]
        .iter()
        .any(|m| c.contains(m))
}

pub fn is_hybrid_cpu(cpu: &str) -> bool {
    let c = cpu.to_ascii_lowercase();
    if !c.contains("intel") {
        return false;
    }
    if c.contains("core(tm) ultra") || c.contains("core ultra") {
        return true;
    }
    // 12th–14th gen Core i3/i5/i7/i9 (e.g. i7-12700K, i5-13400F). The
    // i3-12100 family has no E-cores but nothing here hinges on that.
    c.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-')
        .filter_map(|tok| tok.strip_prefix('i').and_then(|r| r.split_once('-')))
        .any(|(tier, model)| {
            matches!(tier, "3" | "5" | "7" | "9")
                && model.len() >= 5
                && matches!(&model[..2], "12" | "13" | "14")
        })
}

pub fn is_dedicated_gpu(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("nvidia")
        || n.contains("radeon rx")
        || n.contains("radeon pro")
        || n.contains("arc(tm) a")
        || n.contains("arc a")
        || n.contains("arc(tm) b")
        || n.contains("arc b")
}

impl System {
    pub fn facts(&self) -> Facts {
        Facts {
            build: self.build,
            has_battery: self.has_battery,
            high_perf_plan_available: self
                .plans
                .iter()
                .any(|p| p.eq_ignore_ascii_case(PLAN_HIGH_PERFORMANCE)),
            dedicated_gpu: self.gpus.iter().any(|(n, _, _)| is_dedicated_gpu(n)),
            has_hdd: self.has_hdd,
            dual_ccd_x3d: is_dual_ccd_x3d(&self.cpu_name),
            hybrid_cpu: is_hybrid_cpu(&self.cpu_name),
            system_free_pct: self.system_free_pct,
        }
    }
}

/// Microsoft lifecycle end-of-servicing for Home/Pro editions; Enterprise and
/// Education get 12 more months. Returns (version label, end date).
pub fn windows_servicing(build: u32, edition: &str) -> Option<(&'static str, chrono::NaiveDate)> {
    let (label, y, mo, d) = match build {
        19045 => ("Windows 10 22H2", 2025, 10, 14),
        22621 => ("Windows 11 22H2", 2024, 10, 8),
        22631 => ("Windows 11 23H2", 2025, 11, 11),
        26100 => ("Windows 11 24H2", 2026, 10, 13),
        26200 => ("Windows 11 25H2", 2027, 10, 12),
        _ => return None,
    };
    let enterprise = edition.contains("Enterprise") || edition.contains("Education");
    let year = if enterprise && build != 19045 {
        y + 1
    } else {
        y
    };
    chrono::NaiveDate::from_ymd_opt(year, mo, d).map(|date| (label, date))
}

// ── Diagnosis (pure) ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Diagnosis {
    /// Applicable, beneficial setting already in place.
    Configured,
    /// Applicable and justified; offered prominently.
    Recommended,
    /// Applicable, but only useful in specific situations; user's choice.
    Available,
    /// Requirements not met on this system.
    NotApplicable,
    /// Applied, but harmful/outdated or unsuitable for this system.
    Review,
    /// Applied, executes correctly, no proven gain.
    AppliedNoBenefit,
    /// Not offered: no proven benefit, outdated, harmful or unverified.
    NotRecommended,
    /// The setting does not exist on this system.
    Unsupported,
    /// Applied by the app earlier, but Windows no longer has the setting.
    Drifted,
    /// A previous apply did not finish.
    Failed,
    /// Covered by another applied tweak.
    Redundant,
    /// State cannot be read.
    Unknown,
}

impl Diagnosis {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Recommended => "recommended",
            Self::Available => "available",
            Self::NotApplicable => "notApplicable",
            Self::Review => "review",
            Self::AppliedNoBenefit => "appliedNoBenefit",
            Self::NotRecommended => "notRecommended",
            Self::Unsupported => "unsupported",
            Self::Drifted => "drifted",
            Self::Failed => "failed",
            Self::Redundant => "redundant",
            Self::Unknown => "unknown",
        }
    }
}

/// Journal facts about one tweak.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JournalState {
    /// A completed, not reverted entry exists.
    pub open: bool,
    /// An entry was started but never completed and not reverted.
    pub incomplete: bool,
}

pub fn journal_state(journal: &[JournalEntry], id: &str) -> JournalState {
    let mine = journal.iter().filter(|e| e.tweak_id == id && !e.reverted);
    let mut s = JournalState::default();
    for e in mine {
        if e.completed {
            s.open = true;
        } else if e.attempted > 0 {
            s.incomplete = true;
        }
    }
    s
}

pub struct Input<'a> {
    pub meta: &'a OptMeta,
    /// Live status: applied | partial | not_applied | unsupported | unknown.
    pub status: &'a str,
    pub journal: JournalState,
    pub facts: &'a Facts,
    /// Ids of tweaks currently applied (live or journal).
    pub applied_ids: &'a [&'a str],
    /// A higher-performance equivalent is already active (Ultimate plan).
    pub equivalent_active: bool,
}

/// The per-tweak verdict and a short reason the UI can show verbatim.
pub fn diagnose(i: &Input) -> (Diagnosis, String) {
    use Diagnosis::*;
    let m = i.meta;
    let applied = i.status == "applied";
    let ineligible = optregistry::ineligible_reason(m, i.facts);
    let offerable = optregistry::offerable(m.evidence);

    if i.journal.incomplete && !applied {
        return (
            Failed,
            "A previous attempt did not finish. Check the undo history.".into(),
        );
    }
    if i.status == "unsupported" {
        return (
            Unsupported,
            "This setting does not exist on this system.".into(),
        );
    }
    if i.journal.open && matches!(i.status, "not_applied" | "partial") {
        return (
            Drifted,
            "Applied by this app earlier, but Windows no longer has that value (changed by Windows, a driver, or another tool). Undo the old entry before applying again.".into(),
        );
    }
    if applied || i.status == "partial" {
        if let Some(reason) = ineligible {
            return (Review, format!("Applied, but not suitable here: {reason}"));
        }
        return match m.evidence {
            Evidence::PotentiallyHarmful | Evidence::Outdated | Evidence::Broken => (
                Review,
                format!(
                    "Applied, but not recommended: {} Consider undoing it.",
                    m.when
                ),
            ),
            Evidence::NoProvenBenefit | Evidence::Unverified => (
                AppliedNoBenefit,
                "Applied. No proven performance benefit; keep it only if you want its side effect."
                    .into(),
            ),
            _ if i.status == "partial" => (
                Drifted,
                "Only some of its values are set. Undo and apply again to repair it.".into(),
            ),
            _ => (Configured, "Correctly configured.".into()),
        };
    }
    if i.equivalent_active {
        return (
            Configured,
            "An equivalent or stronger setting is already active.".into(),
        );
    }
    if let Some(other) = m.redundant_with.iter().find(|r| i.applied_ids.contains(r)) {
        return (
            Redundant,
            format!("Already covered by the applied \"{other}\" tweak."),
        );
    }
    if let Some(reason) = ineligible {
        return (NotApplicable, reason.into());
    }
    if !offerable {
        return (NotRecommended, m.when.into());
    }
    if i.status == "unknown" {
        return (
            Unknown,
            "The current state cannot be read on this system.".into(),
        );
    }
    if m.recommend {
        (Recommended, m.when.into())
    } else {
        (Available, m.when.into())
    }
}

/// Whether "Optimize My PC" may apply this item (the UI still asks first).
pub fn one_click_eligible(
    m: &OptMeta,
    d: Diagnosis,
    admin: bool,
    requires_admin: bool,
) -> Result<(), &'static str> {
    if !m.one_click || !optregistry::offerable(m.evidence) {
        return Err("Not part of one-click optimization.");
    }
    if d != Diagnosis::Recommended {
        return Err("Not needed on this system.");
    }
    if requires_admin && !admin {
        return Err("Needs administrator rights.");
    }
    Ok(())
}

// ── Apply outcome (pure) ─────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Verified,
    VerifiedNeedsRestart,
    Already,
    Skipped,
    Failed,
    Unverified,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::VerifiedNeedsRestart => "needsRestart",
            Self::Already => "already",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::Unverified => "unverified",
        }
    }
}

/// Classifies a finished apply by reading the setting back.
pub fn classify_after_apply(reboot: Reboot, readback: (&str, &str)) -> (Outcome, String) {
    match readback {
        ("applied", "live") => match reboot {
            Reboot::Restart => (
                Outcome::VerifiedNeedsRestart,
                "The setting was confirmed in Windows; it takes effect after a restart.".into(),
            ),
            Reboot::SignOut => (
                Outcome::Verified,
                "The setting was confirmed in Windows; some programs pick it up after you sign out.".into(),
            ),
            _ => (Outcome::Verified, "The setting was confirmed in Windows.".into()),
        },
        ("applied", _) => (
            Outcome::Unverified,
            "The command ran, but Windows offers no way to read this setting back.".into(),
        ),
        (status, _) => (
            Outcome::Failed,
            format!("The change could not be confirmed: Windows reports \"{status}\". Use Undo to restore the previous values."),
        ),
    }
}

// ── System checks beyond the tweak catalog (pure) ────────────────────────────

fn check(id: &str, severity: &str, title: String, detail: String, tool: Option<&str>) -> Value {
    json!({ "id": id, "severity": severity, "title": title, "detail": detail, "tool": tool })
}

pub fn system_checks(s: &System, journal: &[JournalEntry], today: chrono::NaiveDate) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some((label, end)) = windows_servicing(s.build, &s.edition) {
        let days = (end - today).num_days();
        if days < 0 {
            out.push(check("windows_eos", "warning",
                format!("{label} no longer receives security updates"),
                format!("Support for this version ended on {end}. Update to a supported Windows version; no tweak can replace security updates."),
                Some("updates")));
        } else if days <= 90 {
            out.push(check(
                "windows_eos_soon",
                "info",
                format!("{label} support ends on {end}"),
                "Plan the update to the next Windows version.".into(),
                Some("updates"),
            ));
        }
    }
    if let Some(free) = s.system_free_pct {
        if free < 10.0 {
            out.push(check("disk_space", "warning",
                format!("System drive almost full ({free:.0}% free)"),
                "Below about 10% free space, Windows updates and the page file can fail. Free up space with Cleanup or Disk analyzer.".into(),
                Some("cleanup")));
        }
    }
    if s.pagefile_disabled {
        out.push(check("pagefile_off", "warning", "The page file is disabled".into(),
            "Without a page file, programs can crash when memory runs out and no crash dumps are written. Re-enable automatic management under Advanced perf tweaks.".into(),
            Some("perftweaks")));
    }
    if s.defender_realtime == Some(false) {
        out.push(check("defender_off", "warning", "Microsoft Defender real-time protection is off".into(),
            "Turn it back on unless another antivirus is active. It is not a meaningful gaming bottleneck on modern PCs.".into(),
            Some("security")));
    }
    if !s.has_battery
        && s.active_plan
            .as_deref()
            .is_some_and(|p| p.eq_ignore_ascii_case(PLAN_POWER_SAVER))
    {
        out.push(check("power_saver", "warning", "Power saver plan is active on a desktop".into(),
            "Power saver limits CPU clocks. Switch to Balanced or High performance under Power plan.".into(),
            Some("powerplan")));
    }
    if s.system_disk_hdd {
        out.push(check("system_hdd", "info", "Windows runs from a hard disk".into(),
            "A hard disk is the biggest bottleneck for boot and load times. No setting can fix that; an SSD can.".into(),
            None));
    }
    if s.ram_mb > 0 && s.ram_mb < 8 * 1024 {
        out.push(check(
            "low_ram",
            "info",
            format!("{} GB of RAM", s.ram_mb / 1024),
            "Current games commonly need 16 GB. No setting can replace memory.".into(),
            None,
        ));
    }
    for (name, _, date) in &s.gpus {
        if let Ok(d) = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
            if is_dedicated_gpu(name) && (today - d).num_days() > 365 {
                out.push(check("gpu_driver_old", "info",
                    format!("{name}: driver from {date}"),
                    "The graphics driver is more than a year old. New drivers often fix game issues; update from the vendor or the Drivers tool.".into(),
                    Some("drivers")));
            }
        }
    }
    let incomplete = journal
        .iter()
        .filter(|e| !e.reverted && !e.completed && e.attempted > 0)
        .count();
    if incomplete > 0 {
        out.push(check(
            "incomplete_changes",
            "warning",
            format!("{incomplete} change(s) did not finish"),
            "A previous apply was interrupted. Open the undo history to restore those entries."
                .into(),
            Some("reports"),
        ));
    }
    out
}

// ── I/O layer ────────────────────────────────────────────────────────────────

const FACTS_PS: &str = r#"
$o = [ordered]@{}
$cv = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
$o.build = [int]$cv.CurrentBuild; $o.ubr = [int]$cv.UBR
$o.displayVersion = [string]$cv.DisplayVersion; $o.edition = [string]$cv.EditionID
try { $c = Get-CimInstance Win32_Processor -ErrorAction Stop | Select-Object -First 1
      $o.cpu = @{ name = ($c.Name.Trim() -replace '\s+', ' '); cores = [int]$c.NumberOfCores; threads = [int]$c.NumberOfLogicalProcessors } } catch {}
$o.gpus = @(Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -notmatch 'Microsoft Basic|Remote Display|Virtual|Parsec|Meta Virtual' } |
    ForEach-Object { @{ name = $_.Name.Trim(); driver = [string]$_.DriverVersion;
        date = $(if ($_.DriverDate) { $_.DriverDate.ToString('yyyy-MM-dd') } else { '' }) } })
try { $cs = Get-CimInstance Win32_ComputerSystem -ErrorAction Stop
      $o.ramMb = [int64]($cs.TotalPhysicalMemory / 1MB)
      $o.pagefileAuto = [bool]$cs.AutomaticManagedPagefile } catch {}
$o.pagefiles = @(Get-CimInstance Win32_PageFileUsage -ErrorAction SilentlyContinue).Count
$o.hasBattery = @(Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue).Count -gt 0
try {
    $sysDisk = (Get-Partition -DriveLetter $env:SystemDrive.Substring(0,1) -ErrorAction Stop | Get-Disk).Number
    $pd = @(Get-PhysicalDisk -ErrorAction Stop)
    $o.hasHdd = @($pd | Where-Object { [string]$_.MediaType -eq 'HDD' }).Count -gt 0
    $o.systemDiskHdd = @($pd | Where-Object { [string]$_.DeviceId -eq [string]$sysDisk -and [string]$_.MediaType -eq 'HDD' }).Count -gt 0
} catch {}
try { $v = Get-Volume -DriveLetter $env:SystemDrive.Substring(0,1) -ErrorAction Stop
      if ($v.Size -gt 0) { $o.systemFreePct = [math]::Round($v.SizeRemaining / $v.Size * 100, 1) } } catch {}
try { $mp = Get-MpComputerStatus -ErrorAction Stop
      if ($mp.AMServiceEnabled) { $o.defenderRealtime = [bool]$mp.RealTimeProtectionEnabled } } catch {}
$o | ConvertTo-Json -Compress -Depth 4
"#;

fn guids(out: &str) -> Vec<String> {
    out.split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .filter(|t| crate::ps::is_guid(t))
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

#[cfg(windows)]
pub fn detect() -> Result<System, String> {
    let v = crate::ps::run_json(FACTS_PS)?;
    let s = |k: &str| v[k].as_str().unwrap_or_default().to_string();
    let mut sys = System {
        build: v["build"].as_u64().unwrap_or(0) as u32,
        ubr: v["ubr"].as_u64().unwrap_or(0) as u32,
        display_version: s("displayVersion"),
        edition: s("edition"),
        cpu_name: v["cpu"]["name"].as_str().unwrap_or_default().to_string(),
        cpu_cores: v["cpu"]["cores"].as_u64().unwrap_or(0) as u32,
        cpu_threads: v["cpu"]["threads"].as_u64().unwrap_or(0) as u32,
        ram_mb: v["ramMb"].as_u64().unwrap_or(0),
        has_battery: v["hasBattery"].as_bool().unwrap_or(false),
        has_hdd: v["hasHdd"].as_bool().unwrap_or(false),
        system_disk_hdd: v["systemDiskHdd"].as_bool().unwrap_or(false),
        system_free_pct: v["systemFreePct"].as_f64(),
        pagefile_disabled: v["pagefileAuto"].as_bool() == Some(false)
            && v["pagefiles"].as_u64() == Some(0),
        defender_realtime: v["defenderRealtime"].as_bool(),
        ..Default::default()
    };
    let gpus = match &v["gpus"] {
        Value::Array(a) => a.clone(),
        o @ Value::Object(_) => vec![o.clone()],
        _ => vec![],
    };
    sys.gpus = gpus
        .iter()
        .map(|g| {
            let f = |k: &str| g[k].as_str().unwrap_or_default().to_string();
            (f("name"), f("driver"), f("date"))
        })
        .collect();
    if let Ok(out) = crate::ps::exec("powercfg.exe", &["/list"]) {
        sys.plans = guids(&out);
    }
    if let Ok(out) = crate::ps::exec("powercfg.exe", &["/getactivescheme"]) {
        sys.active_plan = guids(&out).into_iter().next();
    }
    sys.ultimate_active = match (
        &sys.active_plan,
        crate::gameprofile::resolve_ultimate_plan(),
    ) {
        (Some(active), Some(ultimate)) => active.eq_ignore_ascii_case(&ultimate),
        _ => false,
    };
    Ok(sys)
}

fn system_json(s: &System) -> Value {
    let f = s.facts();
    json!({
        "windows": {
            "build": s.build, "ubr": s.ubr, "version": s.display_version, "edition": s.edition,
            "isWindows11": s.build >= 22000,
        },
        "cpu": { "name": s.cpu_name, "cores": s.cpu_cores, "threads": s.cpu_threads,
                 "hybrid": f.hybrid_cpu, "dualCcdX3d": f.dual_ccd_x3d },
        "gpus": s.gpus.iter().map(|(n, d, date)| json!({
            "name": n, "driver": d, "driverDate": date, "dedicated": is_dedicated_gpu(n),
        })).collect::<Vec<_>>(),
        "ramMb": s.ram_mb,
        "isLaptop": s.has_battery,
        "storage": { "hasHdd": s.has_hdd, "systemDiskHdd": s.system_disk_hdd, "systemFreePct": s.system_free_pct },
        "power": { "activePlan": s.active_plan, "highPerformanceAvailable": f.high_perf_plan_available,
                   "ultimateActive": s.ultimate_active },
    })
}

struct Item {
    meta: &'static OptMeta,
    tweak: Value,
    status: String,
    source: String,
    diagnosis: Diagnosis,
    reason: String,
}

fn evaluate(s: &System, journal: &[JournalEntry]) -> Vec<Item> {
    let facts = s.facts();
    let tweaks = crate::tweaks::list_with_status();
    let list = tweaks.as_array().cloned().unwrap_or_default();
    let applied_ids: Vec<&str> = list
        .iter()
        .filter(|t| t["status"] == "applied")
        .filter_map(|t| t["id"].as_str())
        .collect();
    list.iter()
        .filter_map(|t| {
            let id = t["id"].as_str()?;
            let meta = optregistry::meta(id)?;
            let status = t["status"].as_str().unwrap_or("unknown");
            let equivalent_active = id == "power_high_performance" && s.ultimate_active;
            let (diagnosis, reason) = diagnose(&Input {
                meta,
                status,
                journal: journal_state(journal, id),
                facts: &facts,
                applied_ids: &applied_ids,
                equivalent_active,
            });
            Some(Item {
                meta,
                tweak: t.clone(),
                status: status.to_string(),
                source: t["statusSource"].as_str().unwrap_or("none").to_string(),
                diagnosis,
                reason,
            })
        })
        .collect()
}

fn item_json(it: &Item, admin: bool) -> Value {
    let t = &it.tweak;
    let requires_admin = t["requiresAdmin"].as_bool().unwrap_or(false);
    let one_click = one_click_eligible(it.meta, it.diagnosis, admin, requires_admin);
    json!({
        "id": it.meta.id,
        "name": t["name"], "description": t["description"], "risk": t["risk"],
        "requiresAdmin": requires_admin, "canUndo": t["canUndo"],
        "status": it.status, "statusSource": it.source,
        "evidence": it.meta.evidence, "purpose": it.meta.purpose, "group": it.meta.group,
        "benefit": it.meta.benefit, "when": it.meta.when, "reboot": it.meta.reboot,
        "diagnosis": it.diagnosis.as_str(), "reason": it.reason,
        "oneClick": one_click.is_ok(),
        "oneClickCandidate": it.meta.one_click,
        "oneClickBlocked": one_click.err(),
    })
}

/// Full health check: detected system, every tweak's verdict, system checks.
pub fn scan() -> Value {
    #[cfg(windows)]
    {
        let system = match detect() {
            Ok(s) => s,
            Err(e) => return json!({ "error": format!("System detection failed: {e}") }),
        };
        let admin = crate::ps::is_admin();
        let journal = crate::safety::load_journal().unwrap_or_default();
        let items = evaluate(&system, &journal);
        let checks = system_checks(&system, &journal, chrono::Local::now().date_naive());
        let count = |d: Diagnosis| items.iter().filter(|i| i.diagnosis == d).count();
        let summary = json!({
            "configured": count(Diagnosis::Configured),
            "recommended": count(Diagnosis::Recommended),
            "available": count(Diagnosis::Available),
            "review": count(Diagnosis::Review) + count(Diagnosis::Drifted),
            "failed": count(Diagnosis::Failed),
            "notApplicable": count(Diagnosis::NotApplicable) + count(Diagnosis::Unsupported),
            "attention": checks.iter().filter(|c| c["severity"] == "warning").count(),
        });
        json!({
            "time": chrono::Local::now().to_rfc3339(),
            "admin": admin,
            "system": system_json(&system),
            "summary": summary,
            "checks": checks,
            "items": items.iter().map(|i| item_json(i, admin)).collect::<Vec<_>>(),
        })
    }
    #[cfg(not(windows))]
    json!({ "error": "The health check is only available on Windows." })
}

/// Applies the approved ids (one-click path): each id is re-checked against a
/// fresh scan, applied through the journaled engine, then read back.
pub fn apply(ids: Vec<String>, create_restore_point: bool) -> Value {
    #[cfg(windows)]
    {
        let system = match detect() {
            Ok(s) => s,
            Err(e) => return json!({ "error": format!("System detection failed: {e}") }),
        };
        let admin = crate::ps::is_admin();
        let journal = crate::safety::load_journal().unwrap_or_default();
        let items = evaluate(&system, &journal);

        let restore_point = if create_restore_point && admin && !ids.is_empty() {
            Some(
                match crate::safety::create_restore_point("AD HyperOptimize: Optimize My PC") {
                    Ok(m) => json!({ "ok": true, "message": m }),
                    Err(e) => json!({ "ok": false, "message": e }),
                },
            )
        } else {
            None
        };

        let mut results = Vec::new();
        for id in &ids {
            let Some(it) = items.iter().find(|i| i.meta.id == id) else {
                results.push(json!({ "id": id, "name": id, "outcome": "failed", "detail": "Unknown optimization." }));
                continue;
            };
            let name = it.tweak["name"].clone();
            let requires_admin = it.tweak["requiresAdmin"].as_bool().unwrap_or(false);
            if it.diagnosis == Diagnosis::Configured {
                results.push(json!({ "id": id, "name": name, "outcome": Outcome::Already.as_str(), "detail": it.reason }));
                continue;
            }
            if let Err(why) = one_click_eligible(it.meta, it.diagnosis, admin, requires_admin) {
                results.push(json!({ "id": id, "name": name, "outcome": Outcome::Skipped.as_str(), "detail": why }));
                continue;
            }
            let (outcome, detail) = match crate::tweaks::apply(id) {
                Err(e) => (Outcome::Failed, e),
                Ok(v) if v["status"] == "already_applied" => {
                    (Outcome::Already, "Already applied.".to_string())
                }
                Ok(_) => match crate::tweaks::status_of(id) {
                    Some(readback) => classify_after_apply(it.meta.reboot, readback),
                    None => (
                        Outcome::Unverified,
                        "The setting could not be read back.".into(),
                    ),
                },
            };
            results.push(
                json!({ "id": id, "name": name, "outcome": outcome.as_str(), "detail": detail }),
            );
        }
        let count = |o: Outcome| {
            results
                .iter()
                .filter(|r| r["outcome"] == o.as_str())
                .count()
        };
        json!({
            "restorePoint": restore_point,
            "results": results,
            "summary": {
                "verified": count(Outcome::Verified),
                "needsRestart": count(Outcome::VerifiedNeedsRestart),
                "already": count(Outcome::Already),
                "skipped": count(Outcome::Skipped),
                "failed": count(Outcome::Failed),
                "unverified": count(Outcome::Unverified),
            },
        })
    }
    #[cfg(not(windows))]
    {
        let _ = (ids, create_restore_point);
        json!({ "error": "Only available on Windows." })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::optregistry::meta;

    fn desktop() -> Facts {
        Facts {
            build: 26200,
            high_perf_plan_available: true,
            dedicated_gpu: true,
            system_free_pct: Some(40.0),
            ..Default::default()
        }
    }

    fn diag(id: &str, status: &str, journal: JournalState, facts: &Facts) -> Diagnosis {
        diagnose(&Input {
            meta: meta(id).unwrap(),
            status,
            journal,
            facts,
            applied_ids: &[],
            equivalent_active: false,
        })
        .0
    }

    #[test]
    fn cpu_and_gpu_model_rules() {
        assert!(is_dual_ccd_x3d("AMD Ryzen 9 7950X3D 16-Core Processor"));
        assert!(!is_dual_ccd_x3d("AMD Ryzen 7 7800X3D 8-Core Processor"));
        assert!(is_hybrid_cpu("12th Gen Intel(R) Core(TM) i7-12700K"));
        assert!(is_hybrid_cpu("13th Gen Intel(R) Core(TM) i5-13400F"));
        assert!(is_hybrid_cpu("Intel(R) Core(TM) Ultra 7 155H"));
        assert!(!is_hybrid_cpu("Intel(R) Core(TM) i7-9700K CPU @ 3.60GHz"));
        assert!(!is_hybrid_cpu("AMD Ryzen 7 5800X 8-Core Processor"));
        assert!(is_dedicated_gpu("NVIDIA GeForce RTX 4070"));
        assert!(is_dedicated_gpu("AMD Radeon RX 6800"));
        assert!(is_dedicated_gpu("Intel(R) Arc(TM) A770 Graphics"));
        assert!(!is_dedicated_gpu("Intel(R) Arc(TM) Graphics")); // Core Ultra iGPU
        assert!(!is_dedicated_gpu("AMD Radeon(TM) Graphics")); // APU iGPU
        assert!(!is_dedicated_gpu("Intel(R) UHD Graphics 770"));
    }

    #[test]
    fn harmful_and_unproven_tweaks_are_never_recommended() {
        let f = desktop();
        for m in optregistry::REGISTRY {
            let d = diag(m.id, "not_applied", JournalState::default(), &f);
            if !optregistry::offerable(m.evidence) {
                assert!(
                    matches!(d, Diagnosis::NotRecommended | Diagnosis::NotApplicable),
                    "{} -> {d:?}",
                    m.id
                );
            }
            assert!(
                one_click_eligible(m, d, true, true).is_err() || m.one_click,
                "{} offered in one click",
                m.id
            );
        }
    }

    #[test]
    fn applied_states_are_classified() {
        let f = desktop();
        let none = JournalState::default();
        assert_eq!(
            diag("game_mode_on", "applied", none, &f),
            Diagnosis::Configured
        );
        assert_eq!(
            diag("game_mode_on", "not_applied", none, &f),
            Diagnosis::Recommended
        );
        assert_eq!(
            diag("core_parking_off", "applied", none, &f),
            Diagnosis::Review
        );
        assert_eq!(
            diag("nagle_off", "applied", none, &f),
            Diagnosis::AppliedNoBenefit
        );
        assert_eq!(
            diag("hags_on", "not_applied", none, &f),
            Diagnosis::Available
        );
        assert_eq!(
            diag("svc_fax_off", "unsupported", none, &f),
            Diagnosis::Unsupported
        );
        assert_eq!(
            diag("tcp_autotuning_normal", "unknown", none, &f),
            Diagnosis::Unknown
        );
        let laptop = Facts {
            has_battery: true,
            ..desktop()
        };
        assert_eq!(
            diag("power_high_performance", "not_applied", none, &laptop),
            Diagnosis::NotApplicable
        );
        assert_eq!(
            diag("power_high_performance", "applied", none, &laptop),
            Diagnosis::Review
        );
    }

    #[test]
    fn journal_drift_and_failures() {
        let f = desktop();
        let open = JournalState {
            open: true,
            incomplete: false,
        };
        let incomplete = JournalState {
            open: false,
            incomplete: true,
        };
        assert_eq!(
            diag("game_dvr_off", "not_applied", open, &f),
            Diagnosis::Drifted
        );
        assert_eq!(
            diag("game_dvr_off", "not_applied", incomplete, &f),
            Diagnosis::Failed
        );
        // A state that reads back as applied wins over a stale incomplete entry.
        assert_eq!(
            diag("game_dvr_off", "applied", incomplete, &f),
            Diagnosis::Configured
        );
    }

    #[test]
    fn equivalents_and_redundancy() {
        let f = desktop();
        let d = diagnose(&Input {
            meta: meta("power_high_performance").unwrap(),
            status: "not_applied",
            journal: JournalState::default(),
            facts: &f,
            applied_ids: &[],
            equivalent_active: true,
        });
        assert_eq!(d.0, Diagnosis::Configured);
        let d = diagnose(&Input {
            meta: meta("fast_startup_off").unwrap(),
            status: "not_applied",
            journal: JournalState::default(),
            facts: &f,
            applied_ids: &["hibernate_off"],
            equivalent_active: false,
        });
        assert_eq!(d.0, Diagnosis::Redundant);
    }

    #[test]
    fn one_click_requires_admin_when_needed() {
        let m = meta("tcp_autotuning_normal").unwrap();
        assert!(one_click_eligible(m, Diagnosis::Recommended, true, true).is_ok());
        assert_eq!(
            one_click_eligible(m, Diagnosis::Recommended, false, true),
            Err("Needs administrator rights.")
        );
        assert!(one_click_eligible(m, Diagnosis::Configured, true, true).is_err());
        let not_one_click = meta("mouse_accel_off").unwrap();
        assert!(one_click_eligible(not_one_click, Diagnosis::Recommended, true, false).is_err());
    }

    #[test]
    fn verification_never_reports_unconfirmed_success() {
        assert_eq!(
            classify_after_apply(Reboot::NotNeeded, ("applied", "live")).0,
            Outcome::Verified
        );
        assert_eq!(
            classify_after_apply(Reboot::Restart, ("applied", "live")).0,
            Outcome::VerifiedNeedsRestart
        );
        assert_eq!(
            classify_after_apply(Reboot::NotNeeded, ("applied", "journal")).0,
            Outcome::Unverified
        );
        assert_eq!(
            classify_after_apply(Reboot::NotNeeded, ("not_applied", "live")).0,
            Outcome::Failed
        );
        assert_eq!(
            classify_after_apply(Reboot::NotNeeded, ("partial", "live")).0,
            Outcome::Failed
        );
    }

    #[test]
    fn servicing_and_system_checks() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        let win10 = System {
            build: 19045,
            edition: "Professional".into(),
            ..Default::default()
        };
        let ids = |s: &System| -> Vec<String> {
            system_checks(s, &[], today)
                .iter()
                .map(|c| c["id"].as_str().unwrap().to_string())
                .collect()
        };
        assert!(ids(&win10).contains(&"windows_eos".to_string()));
        let win11 = System {
            build: 26200,
            edition: "Professional".into(),
            ..Default::default()
        };
        assert!(ids(&win11).is_empty());
        let r24h2_ent = windows_servicing(26100, "Enterprise").unwrap().1;
        assert_eq!(
            r24h2_ent,
            chrono::NaiveDate::from_ymd_opt(2027, 10, 13).unwrap()
        );
        let bad = System {
            build: 26200,
            system_free_pct: Some(5.0),
            pagefile_disabled: true,
            defender_realtime: Some(false),
            active_plan: Some(PLAN_POWER_SAVER.into()),
            ..Default::default()
        };
        let got = ids(&bad);
        for want in ["disk_space", "pagefile_off", "defender_off", "power_saver"] {
            assert!(got.contains(&want.to_string()), "{want} in {got:?}");
        }
        // Power saver on a laptop is the user's choice, not an issue.
        let laptop = System {
            has_battery: true,
            ..bad
        };
        assert!(!ids(&laptop).contains(&"power_saver".to_string()));
    }

    /// Runs the full health check against this machine (read-only: registry,
    /// WMI and powercfg reads) and prints the verdicts.
    /// `cargo test --locked -- --ignored real_health_scan --nocapture`
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn real_health_scan() {
        let v = scan();
        assert!(v.get("error").is_none(), "{v}");
        if let Ok(path) = std::env::var("ADHO_SCAN_OUT") {
            std::fs::write(path, v.to_string()).unwrap();
        }
        println!("SYSTEM {}", v["system"]);
        println!("SUMMARY {}", v["summary"]);
        for c in v["checks"].as_array().unwrap() {
            println!("CHECK {} {} | {}", c["severity"], c["id"], c["title"]);
        }
        let items = v["items"].as_array().unwrap();
        assert_eq!(items.len(), optregistry::REGISTRY.len());
        for i in items {
            println!(
                "ITEM {:<26} {:<16} status={}/{} oneClick={}",
                i["id"].as_str().unwrap(),
                i["diagnosis"].as_str().unwrap(),
                i["status"].as_str().unwrap(),
                i["statusSource"].as_str().unwrap(),
                i["oneClick"]
            );
        }
    }

    #[test]
    fn power_plan_guids_are_parsed() {
        let out =
            "GUID des Energieschemas: 381b4222-f694-41f0-9685-ff5bb260df2e  (Ausbalanciert) *";
        assert_eq!(
            guids(out),
            vec!["381b4222-f694-41f0-9685-ff5bb260df2e".to_string()]
        );
    }
}
