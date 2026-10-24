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
    pub ram_modules: Vec<RamModule>,
    pub displays: Vec<Display>,
    /// HKCU DirectXUserGlobalSettings; None when the value does not exist.
    pub dx_settings: Option<String>,
    /// (BAR1 total MiB, framebuffer total MiB) from nvidia-smi.
    pub nvidia_bar: Option<(u64, u64)>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RamModule {
    pub part: String,
    /// Win32_PhysicalMemory.ConfiguredClockSpeed (MT/s on current firmware).
    pub configured: u32,
    /// SMBIOS memory type: 26 = DDR4, 34 = DDR5.
    pub smbios_type: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Display {
    pub monitor: String,
    pub width: u32,
    pub height: u32,
    pub current_hz: u32,
    /// Highest refresh rate the monitor/driver offers at the current resolution.
    pub max_hz_same_res: u32,
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
    /// Retired tweak (no measurable benefit) that is still active: reset it.
    RetiredActive,
    /// Retired tweak that is not active: not shown.
    Retired,
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
            Self::RetiredActive => "retiredActive",
            Self::Retired => "retired",
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
    if optregistry::is_retired(m.id) {
        let at_default =
            applied && !i.journal.open && optregistry::MATCHES_WINDOWS_DEFAULT.contains(&m.id);
        let active = (applied && !at_default) || i.status == "partial" || i.journal.open;
        return if active {
            (
                RetiredActive,
                format!(
                    "Retired: no measurable benefit. {} Reset restores the original values.",
                    m.when
                ),
            )
        } else {
            (Retired, "Retired: no measurable benefit.".into())
        };
    }
    if i.status == "unsupported" {
        return (
            Unsupported,
            "This setting does not exist on this system.".into(),
        );
    }
    // An equivalent or stronger setting (Ultimate plan instead of High
    // performance) is the user's choice, not drift: undoing the old entry
    // would switch the plan back to whatever was active before.
    if i.equivalent_active && i.status == "not_applied" {
        return (
            Configured,
            "An equivalent or stronger setting is already active.".into(),
        );
    }
    if i.journal.open && matches!(i.status, "not_applied" | "partial") {
        let next = if ineligible.is_some() || !offerable {
            "Undo closes the old entry and restores the values from before."
        } else {
            "Undo restores the values from before; apply it again afterwards if you still want it."
        };
        return (
            Drifted,
            format!("Applied by this app earlier, but Windows no longer has that value (changed by Windows, a driver, or another tool). {next}"),
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

/// Classifies a finished reset of a retired tweak by reading the state back.
/// "Still applied" is only accepted for tweaks whose target is the Windows
/// default; anywhere else something re-applied it or the reset did not stick.
pub fn classify_after_reset(id: &str, readback: (&str, &str)) -> (Outcome, String) {
    match readback {
        ("not_applied", "live") => (
            Outcome::Verified,
            "Reset; the original value was confirmed in Windows.".into(),
        ),
        ("applied", _) if optregistry::MATCHES_WINDOWS_DEFAULT.contains(&id) => (
            Outcome::Already,
            "The original value is identical to this setting (Windows default); nothing to change."
                .into(),
        ),
        ("applied", _) => (
            Outcome::Failed,
            "Still active after the reset: another tool, a policy or another page of this app sets it again.".into(),
        ),
        ("partial", _) => (
            Outcome::Failed,
            "Only partly reset. Check the undo history.".into(),
        ),
        _ => (
            Outcome::Unverified,
            "Reset ran; Windows offers no way to read this setting back.".into(),
        ),
    }
}

/// Outcome of one retired-tweak reset: the engine's revert result plus the
/// state read back afterwards. A failed engine revert still counts as reset
/// when Windows confirms the original state: the Debloater duplicate may have
/// held the setting, and after restoring it the engine finds nothing to undo.
pub fn classify_reset(
    id: &str,
    revert: Result<(), String>,
    readback: Option<(&str, &str)>,
) -> (Outcome, String) {
    match (revert, readback) {
        (_, Some(("not_applied", "live"))) => classify_after_reset(id, ("not_applied", "live")),
        (Err(e), _) => (Outcome::Failed, e),
        (Ok(()), Some(rb)) => classify_after_reset(id, rb),
        (Ok(()), None) => (
            Outcome::Unverified,
            "The setting could not be read back.".into(),
        ),
    }
}

// ── Gaming setup checks (pure) ───────────────────────────────────────────────

/// Rated speed encoded in a memory kit's part number, in MT/s. Kingston/HyperX
/// encode it as two digits after the generation (KF436C… = 3600, KF560C… =
/// 6000); most other vendors spell it out (CMK32GX4M2Z3600C18, F4-3600C16…).
/// None when the part number carries no plausible speed.
pub fn ram_rated_from_part(part: &str) -> Option<u32> {
    let p = part.trim().to_ascii_uppercase();
    for prefix in ["KF4", "KF5", "KHX"] {
        if let Some(rest) = p.strip_prefix(prefix) {
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if digits.len() == 2 && rest[2..].starts_with('C') {
                return digits.parse::<u32>().ok().map(|d| d * 100);
            }
        }
    }
    p.split(|c: char| !c.is_ascii_digit())
        .filter(|run| run.len() == 4)
        .filter_map(|run| run.parse::<u32>().ok())
        .filter(|v| (2133..=9600).contains(v))
        .max()
}

fn gaming(id: &str, status: &str, title: String, detail: String, settings: Option<&str>) -> Value {
    json!({ "id": id, "status": status, "title": title, "detail": detail, "settings": settings })
}

pub fn ram_check(modules: &[RamModule]) -> Value {
    let configured: Vec<u32> = modules
        .iter()
        .map(|m| m.configured)
        .filter(|&c| c > 0)
        .collect();
    let Some(&speed) = configured.iter().min() else {
        return gaming(
            "xmp",
            "unknown",
            "Memory speed".into(),
            "Windows did not report the configured memory speed.".into(),
            None,
        );
    };
    let rated = modules
        .iter()
        .filter_map(|m| ram_rated_from_part(&m.part))
        .max();
    let ddr = modules.first().map(|m| m.smbios_type).unwrap_or(0);
    match rated {
        Some(r) if speed + 100 < r => gaming("xmp", "warning",
            format!("Memory runs at {speed} MT/s, the kit is rated for {r} MT/s"),
            "The rated speed comes from the modules' part number. Enable XMP (Intel) or EXPO/DOCP (AMD) in the BIOS, then check stability. Faster memory mainly helps CPU-bound games.".into(),
            None),
        Some(r) => gaming("xmp", "ok",
            format!("Memory runs at its rated {speed} MT/s"),
            format!("Matches the {r} MT/s encoded in the part number, so XMP/EXPO (or equivalent) is active."),
            None),
        None if (ddr == 26 && speed <= 2666) || (ddr == 34 && speed <= 4800) => gaming("xmp", "info",
            format!("Memory runs at {speed} MT/s, a standard base speed"),
            "The part number does not reveal the kit's rating. If the label on your modules shows a higher speed, enable XMP/EXPO in the BIOS.".into(),
            None),
        None => gaming("xmp", "ok", format!("Memory runs at {speed} MT/s"),
            "The part number does not reveal the kit's rating; this speed is above the standard base speeds.".into(),
            None),
    }
}

pub fn refresh_check(displays: &[Display]) -> Value {
    if displays.is_empty() {
        return gaming(
            "refresh",
            "unknown",
            "Display refresh rate".into(),
            "The active displays could not be read.".into(),
            None,
        );
    }
    let below: Vec<&Display> = displays
        .iter()
        .filter(|d| d.max_hz_same_res >= d.current_hz + 10)
        .collect();
    let describe = |d: &Display| {
        format!(
            "{}: {} Hz at {}×{} (up to {} Hz possible)",
            d.monitor, d.current_hz, d.width, d.height, d.max_hz_same_res
        )
    };
    if below.is_empty() {
        gaming(
            "refresh",
            "ok",
            "Displays run at their highest refresh rate".into(),
            displays
                .iter()
                .map(|d| {
                    format!(
                        "{}: {} Hz at {}×{}",
                        d.monitor, d.current_hz, d.width, d.height
                    )
                })
                .collect::<Vec<_>>()
                .join("; "),
            None,
        )
    } else {
        gaming(
            "refresh",
            "warning",
            format!(
                "{} display(s) run below their maximum refresh rate",
                below.len()
            ),
            format!(
                "{}. Choose the highest rate under Settings > Display > Advanced display.",
                below
                    .iter()
                    .map(|d| describe(d))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            Some("display-advanced"),
        )
    }
}

/// `SwapEffectUpgradeEnable` / `VRROptimizeEnable` from the DirectX user
/// settings string (`Key=1;Key=0;`). None = not set (Windows default).
pub fn dx_flag(settings: Option<&str>, key: &str) -> Option<bool> {
    settings?
        .split(';')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case(key))
        .map(|(_, v)| v.trim() == "1")
}

pub fn dx_checks(build: u32, settings: Option<&str>) -> Vec<Value> {
    // Both settings exist from Windows 11 22H2 (build 22621).
    if build < 22621 {
        return vec![];
    }
    let windowed = match dx_flag(settings, "SwapEffectUpgradeEnable") {
        Some(true) => gaming("windowed", "ok", "Optimizations for windowed games: on".into(),
            "DirectX 10/11 games in windowed or borderless mode use the modern presentation model (lower latency, Auto HDR and VRR in windowed mode).".into(),
            None),
        Some(false) => gaming("windowed", "info", "Optimizations for windowed games: off".into(),
            "Turning it on lets DirectX 10/11 games in windowed or borderless mode use the modern presentation model, with lower latency. Turn it off again if a game misbehaves.".into(),
            Some("display-advancedgraphics")),
        None => gaming("windowed", "info", "Optimizations for windowed games: Windows default".into(),
            "Not set explicitly. Check it under Settings > Display > Graphics; it can lower latency for DirectX 10/11 games in windowed or borderless mode.".into(),
            Some("display-advancedgraphics")),
    };
    let vrr = match dx_flag(settings, "VRROptimizeEnable") {
        Some(true) => gaming("vrr", "ok", "Variable refresh rate for all games: on".into(),
            "Games without their own VRR support also use G-SYNC/FreeSync.".into(), None),
        _ => gaming("vrr", "info", "Variable refresh rate for all games: off".into(),
            "Only useful if your monitor supports G-SYNC, FreeSync or VRR: then games without their own VRR support can use it too.".into(),
            Some("display-advancedgraphics")),
    };
    vec![windowed, vrr]
}

/// (BAR1 total, framebuffer total) in MiB from `nvidia-smi -q -d MEMORY`.
pub fn parse_nvidia_bar(out: &str) -> Option<(u64, u64)> {
    let mut section = "";
    let (mut fb, mut bar1) = (None, None);
    for line in out.lines() {
        let t = line.trim();
        if t.starts_with("FB Memory Usage") {
            section = "fb";
        } else if t.starts_with("BAR1 Memory Usage") {
            section = "bar1";
        } else if let Some(v) = t.strip_prefix("Total") {
            let mib = v
                .trim_start_matches([' ', ':'])
                .split_whitespace()
                .next()
                .and_then(|n| n.parse::<u64>().ok());
            match section {
                "fb" if fb.is_none() => fb = mib,
                "bar1" if bar1.is_none() => bar1 = mib,
                _ => {}
            }
            section = "";
        }
    }
    Some((bar1?, fb?))
}

pub fn rebar_check(has_nvidia: bool, bar: Option<(u64, u64)>) -> Value {
    match (has_nvidia, bar) {
        (false, _) => gaming("rebar", "unknown", "Resizable BAR".into(),
            "Only NVIDIA GPUs can be checked here (via nvidia-smi). For AMD, see Smart Access Memory in AMD Software or the BIOS.".into(),
            None),
        (true, None) => gaming("rebar", "unknown", "Resizable BAR".into(),
            "nvidia-smi did not report the BAR1 size.".into(), None),
        (true, Some((bar1, fb))) if bar1 * 10 >= fb * 9 => gaming("rebar", "ok",
            "Resizable BAR is active".into(),
            format!("BAR1 {bar1} MiB covers the {fb} MiB of video memory. The NVIDIA driver uses it per game where it helps."),
            None),
        (true, Some((bar1, _))) => gaming("rebar", "info", "Resizable BAR is off".into(),
            format!("BAR1 is only {bar1} MiB. Enabling \"Above 4G Decoding\" and \"Re-Size BAR Support\" in the BIOS (UEFI boot, CSM off) helps some games; the driver enables it per game only where it helps."),
            None),
    }
}

pub fn gaming_checks(s: &System) -> Vec<Value> {
    let has_nvidia = s
        .gpus
        .iter()
        .any(|(n, _, _)| n.to_ascii_lowercase().contains("nvidia"));
    let mut out = vec![refresh_check(&s.displays), ram_check(&s.ram_modules)];
    out.extend(dx_checks(s.build, s.dx_settings.as_deref()));
    out.push(rebar_check(has_nvidia, s.nvidia_bar));
    out
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

/// Active displays via EnumDisplayDevices/EnumDisplaySettings: the current mode
/// and the highest refresh rate offered at the same resolution. One line per
/// display: monitor|width|height|currentHz|maxHzSameResolution.
const DISPLAYS_PS: &str = r#"
Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class AdhoDisplays {
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DEVMODE {
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmDeviceName;
        public short dmSpecVersion; public short dmDriverVersion; public short dmSize; public short dmDriverExtra;
        public int dmFields; public int dmPositionX; public int dmPositionY; public int dmDisplayOrientation; public int dmDisplayFixedOutput;
        public short dmColor; public short dmDuplex; public short dmYResolution; public short dmTTOption; public short dmCollate;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string dmFormName;
        public short dmLogPixels; public int dmBitsPerPel; public int dmPelsWidth; public int dmPelsHeight;
        public int dmDisplayFlags; public int dmDisplayFrequency; public int dmICMMethod; public int dmICMIntent;
        public int dmMediaType; public int dmDitherType; public int dmReserved1; public int dmReserved2;
        public int dmPanningWidth; public int dmPanningHeight;
    }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    public struct DISPLAY_DEVICE {
        public int cb;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 32)] public string DeviceName;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceString;
        public int StateFlags;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceID;
        [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 128)] public string DeviceKey;
    }
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern bool EnumDisplayDevices(string device, uint index, ref DISPLAY_DEVICE dd, uint flags);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    static extern bool EnumDisplaySettings(string device, int mode, ref DEVMODE dm);
    public static List<string> Report() {
        var lines = new List<string>();
        for (uint i = 0; ; i++) {
            var dd = new DISPLAY_DEVICE(); dd.cb = Marshal.SizeOf(dd);
            if (!EnumDisplayDevices(null, i, ref dd, 0)) break;
            if ((dd.StateFlags & 1) == 0) continue; // not attached to the desktop
            var cur = new DEVMODE(); cur.dmSize = (short)Marshal.SizeOf(cur);
            if (!EnumDisplaySettings(dd.DeviceName, -1, ref cur)) continue;
            int max = cur.dmDisplayFrequency;
            for (int m = 0; ; m++) {
                var mode = new DEVMODE(); mode.dmSize = (short)Marshal.SizeOf(mode);
                if (!EnumDisplaySettings(dd.DeviceName, m, ref mode)) break;
                if (mode.dmPelsWidth == cur.dmPelsWidth && mode.dmPelsHeight == cur.dmPelsHeight && mode.dmDisplayFrequency > max)
                    max = mode.dmDisplayFrequency;
            }
            var mon = new DISPLAY_DEVICE(); mon.cb = Marshal.SizeOf(mon);
            string name = EnumDisplayDevices(dd.DeviceName, 0, ref mon, 0) ? mon.DeviceString : dd.DeviceString;
            lines.Add(name.Replace("|", "/") + "|" + cur.dmPelsWidth + "|" + cur.dmPelsHeight + "|" + cur.dmDisplayFrequency + "|" + max);
        }
        return lines;
    }
}
'@
[AdhoDisplays]::Report()
"#;

pub fn parse_displays(out: &str) -> Vec<Display> {
    out.lines()
        .filter_map(|l| {
            let f: Vec<&str> = l.trim().split('|').collect();
            if f.len() != 5 {
                return None;
            }
            Some(Display {
                monitor: f[0].to_string(),
                width: f[1].parse().ok()?,
                height: f[2].parse().ok()?,
                current_hz: f[3].parse().ok()?,
                max_hz_same_res: f[4].parse().ok()?,
            })
        })
        .collect()
}

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
$o.ram = @(Get-CimInstance Win32_PhysicalMemory -ErrorAction SilentlyContinue | ForEach-Object {
    @{ part = ([string]$_.PartNumber).Trim(); configured = [int]$_.ConfiguredClockSpeed; type = [int]$_.SMBIOSMemoryType } })
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
        ram_modules: match &v["ram"] {
            Value::Array(a) => a.clone(),
            o @ Value::Object(_) => vec![o.clone()],
            _ => vec![],
        }
        .iter()
        .map(|m| RamModule {
            part: m["part"].as_str().unwrap_or_default().to_string(),
            configured: m["configured"].as_u64().unwrap_or(0) as u32,
            smbios_type: m["type"].as_u64().unwrap_or(0) as u32,
        })
        .collect(),
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
    if let Ok(out) = crate::ps::run(DISPLAYS_PS) {
        sys.displays = parse_displays(&out);
    }
    sys.dx_settings = match crate::tweaks::reg_read(
        "HKCU",
        r"Software\Microsoft\DirectX\UserGpuPreferences",
        "DirectXUserGlobalSettings",
    ) {
        Ok(Some(crate::safety::RegVal::Str(v))) => Some(v),
        _ => None,
    };
    // Absolute path: the elevated app must not run a nvidia-smi found on PATH.
    if sys
        .gpus
        .iter()
        .any(|(n, _, _)| n.to_ascii_lowercase().contains("nvidia"))
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let smi = format!(r"{root}\System32\nvidia-smi.exe");
        if let Ok((_, out, _)) = crate::ps::exec_capture(&smi, &["-q", "-d", "MEMORY"]) {
            sys.nvidia_bar = parse_nvidia_bar(&out);
        }
    }
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
        // With the admin helper connected, admin-only items run elevated
        // through it, so they are not blocked here.
        let admin = crate::ps::is_admin() || crate::broker::connected();
        let journal = crate::safety::load_journal().unwrap_or_default();
        let items = evaluate(&system, &journal);
        let checks = system_checks(&system, &journal, chrono::Local::now().date_naive());
        let gaming = gaming_checks(&system);
        let count = |d: Diagnosis| items.iter().filter(|i| i.diagnosis == d).count();
        let summary = json!({
            "configured": count(Diagnosis::Configured),
            "recommended": count(Diagnosis::Recommended),
            "available": count(Diagnosis::Available),
            "review": count(Diagnosis::Review) + count(Diagnosis::Drifted),
            "retiredActive": count(Diagnosis::RetiredActive),
            "failed": count(Diagnosis::Failed),
            "notApplicable": count(Diagnosis::NotApplicable) + count(Diagnosis::Unsupported),
            "attention": checks.iter().filter(|c| c["severity"] == "warning").count()
                + gaming.iter().filter(|c| c["status"] == "warning").count(),
        });
        json!({
            "time": chrono::Local::now().to_rfc3339(),
            "admin": admin,
            "system": system_json(&system),
            "summary": summary,
            "checks": checks,
            "gaming": gaming,
            "items": items.iter().map(|i| item_json(i, admin)).collect::<Vec<_>>(),
        })
    }
    #[cfg(not(windows))]
    json!({ "error": "The health check is only available on Windows." })
}

/// Retired tweaks that the Debloater also offers under its own id and state
/// file: (tweak id, debloater id).
/// `gamedvr_off` also wrote the FSE values of `fse_optimizations_off`.
const DEBLOATER_DUPLICATES: &[(&str, &str)] = &[
    ("cortana_off", "cortana_off"),
    ("fse_optimizations_off", "gamedvr_off"),
];

/// Resets every retired tweak that is still active (journaled undo with the
/// captured originals, else the engine's verified force-revert), then reads
/// each one back.
pub fn reset_retired() -> Value {
    #[cfg(windows)]
    {
        let system = match detect() {
            Ok(s) => s,
            Err(e) => return json!({ "error": format!("System detection failed: {e}") }),
        };
        let admin = crate::ps::is_admin();
        let journal = crate::safety::load_journal().unwrap_or_default();
        let items = evaluate(&system, &journal);
        let mut results = Vec::new();
        for it in items
            .iter()
            .filter(|i| i.diagnosis == Diagnosis::RetiredActive)
        {
            let id = it.meta.id;
            let name = it.tweak["name"].clone();
            if it.tweak["requiresAdmin"].as_bool().unwrap_or(false) && !admin {
                results.push(
                    json!({ "id": id, "name": name, "outcome": Outcome::Skipped.as_str(),
                    "detail": "Needs administrator rights." }),
                );
                continue;
            }
            // The same setting may also be held by a duplicate in another
            // catalog with its own undo state; reset that too.
            // Only with an exact captured state, never the guessed-default undo.
            if let Some(dup) = DEBLOATER_DUPLICATES
                .iter()
                .find(|(t, _)| *t == id)
                .map(|(_, d)| *d)
            {
                if crate::debloater::has_captured_state(dup) {
                    let _ = crate::debloater::revert_tweak(dup.to_string());
                }
            }
            let revert = crate::tweaks::revert(id).map(|_| ());
            let (outcome, detail) = classify_reset(id, revert, crate::tweaks::status_of(id));
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
            "restorePoint": null,
            "results": results,
            "summary": {
                "verified": count(Outcome::Verified),
                "needsRestart": 0,
                "already": 0,
                "skipped": count(Outcome::Skipped),
                "failed": count(Outcome::Failed),
                "unverified": count(Outcome::Unverified),
            },
        })
    }
    #[cfg(not(windows))]
    json!({ "error": "Only available on Windows." })
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
                    matches!(
                        d,
                        Diagnosis::NotRecommended | Diagnosis::NotApplicable | Diagnosis::Retired
                    ),
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
            Diagnosis::RetiredActive
        );
        assert_eq!(
            diag("transparency_off", "applied", none, &f),
            Diagnosis::AppliedNoBenefit
        );
        assert_eq!(
            diag("hags_on", "not_applied", none, &f),
            Diagnosis::Available
        );
        assert_eq!(
            diag("edge_prelaunch_off", "unsupported", none, &f),
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
    fn retired_tweaks_are_hidden_or_offered_for_reset() {
        let f = desktop();
        let none = JournalState::default();
        let open = JournalState {
            open: true,
            incomplete: false,
        };
        assert_eq!(
            diag("nagle_off", "applied", none, &f),
            Diagnosis::RetiredActive
        );
        assert_eq!(
            diag("sysmain_off", "partial", none, &f),
            Diagnosis::RetiredActive
        );
        assert_eq!(
            diag("qos_bandwidth_free", "not_applied", open, &f),
            Diagnosis::RetiredActive
        );
        assert_eq!(
            diag("qos_bandwidth_free", "not_applied", none, &f),
            Diagnosis::Retired
        );
        assert_eq!(
            diag("svc_fax_off", "unsupported", none, &f),
            Diagnosis::Retired
        );
        // Target equals the Windows default and no entry is open: nothing to reset.
        assert_eq!(
            diag("remote_registry_off", "applied", none, &f),
            Diagnosis::Retired
        );
        assert_eq!(
            diag("remote_registry_off", "applied", open, &f),
            Diagnosis::RetiredActive
        );
        assert_eq!(
            diag("xbox_services_off", "partial", none, &f),
            Diagnosis::RetiredActive
        );
        // Not retired: privacy keeps its normal verdicts.
        assert_eq!(
            diag("telemetry_minimal", "applied", none, &f),
            Diagnosis::AppliedNoBenefit
        );
    }

    #[test]
    fn reset_is_only_verified_when_windows_confirms() {
        assert_eq!(
            classify_after_reset("nagle_off", ("not_applied", "live")).0,
            Outcome::Verified
        );
        assert_eq!(
            classify_after_reset("remote_registry_off", ("applied", "live")).0,
            Outcome::Already
        );
        // Anywhere else "still applied" is a failure, never a success.
        assert_eq!(
            classify_after_reset("cortana_off", ("applied", "live")).0,
            Outcome::Failed
        );
        assert_eq!(
            classify_after_reset("nagle_off", ("partial", "live")).0,
            Outcome::Failed
        );
        assert_eq!(
            classify_after_reset("nagle_off", ("unknown", "none")).0,
            Outcome::Unverified
        );
    }

    #[test]
    fn reset_trusts_the_readback_not_the_engine_message() {
        // The Debloater duplicate held the setting: after restoring it the
        // engine has nothing to undo, but Windows confirms the original.
        let nothing = Err("tweak is not currently active, nothing to undo".to_string());
        assert_eq!(
            classify_reset(
                "cortana_off",
                nothing.clone(),
                Some(("not_applied", "live"))
            )
            .0,
            Outcome::Verified
        );
        // A failed revert with the setting still active stays a failure.
        let (o, detail) = classify_reset(
            "cortana_off",
            Err("denied".into()),
            Some(("applied", "live")),
        );
        assert_eq!(o, Outcome::Failed);
        assert_eq!(detail, "denied");
        assert_eq!(
            classify_reset("nagle_off", nothing, None).0,
            Outcome::Failed
        );
        assert_eq!(
            classify_reset("nagle_off", Ok(()), None).0,
            Outcome::Unverified
        );
        assert_eq!(
            classify_reset("cortana_off", Ok(()), Some(("applied", "live"))).0,
            Outcome::Failed
        );
    }

    #[test]
    fn debloater_duplicates_pair_retired_tweaks_with_real_debloater_ids() {
        for (tweak, dup) in DEBLOATER_DUPLICATES {
            assert!(optregistry::is_retired(tweak), "{tweak} is not retired");
            assert!(
                crate::debloater::is_withdrawn(dup),
                "{dup} is still offered"
            );
        }
        assert!(DEBLOATER_DUPLICATES.contains(&("fse_optimizations_off", "gamedvr_off")));
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
    fn ultimate_plan_is_not_drift_of_high_performance() {
        // Old High performance entry still open, user switched to Ultimate.
        let f = desktop();
        let d = diagnose(&Input {
            meta: meta("power_high_performance").unwrap(),
            status: "not_applied",
            journal: JournalState {
                open: true,
                incomplete: false,
            },
            facts: &f,
            applied_ids: &[],
            equivalent_active: true,
        });
        assert_eq!(d.0, Diagnosis::Configured);
        // Without the equivalent it is real drift, and undo is suggested.
        let d = diagnose(&Input {
            meta: meta("power_high_performance").unwrap(),
            status: "not_applied",
            journal: JournalState {
                open: true,
                incomplete: false,
            },
            facts: &f,
            applied_ids: &[],
            equivalent_active: false,
        });
        assert_eq!(d.0, Diagnosis::Drifted);
        assert!(d.1.contains("apply it again"));
    }

    #[test]
    fn drift_of_an_unoffered_tweak_does_not_suggest_reapplying() {
        let f = desktop();
        let d = diagnose(&Input {
            meta: meta("edge_prelaunch_off").unwrap(),
            status: "not_applied",
            journal: JournalState {
                open: true,
                incomplete: false,
            },
            facts: &f,
            applied_ids: &[],
            equivalent_active: false,
        });
        assert_eq!(d.0, Diagnosis::Drifted);
        assert!(!d.1.contains("apply it again"), "{}", d.1);
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
        for c in v["gaming"].as_array().unwrap() {
            println!(
                "GAMING {} {} | {} | {}",
                c["status"], c["id"], c["title"], c["detail"]
            );
        }
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
    fn ram_part_numbers_reveal_rated_speed() {
        assert_eq!(ram_rated_from_part("CMK32GX4M2Z3600C18"), Some(3600));
        assert_eq!(ram_rated_from_part("F4-3600C16D-32GTZN"), Some(3600));
        assert_eq!(ram_rated_from_part("F5-6000J3038F16GX2-TZ5RK"), Some(6000));
        assert_eq!(ram_rated_from_part("KF436C17BBK2/16"), Some(3600));
        assert_eq!(ram_rated_from_part("KF560C40BBK2-32"), Some(6000));
        assert_eq!(ram_rated_from_part("TF3D416G3600HC18J"), Some(3600));
        assert_eq!(ram_rated_from_part("M378A1K43CB2-CTD"), None);
        assert_eq!(ram_rated_from_part("BL2K16G36C16U4B"), None);
    }

    #[test]
    fn xmp_check_compares_configured_with_rated() {
        let m = |part: &str, configured: u32, t: u32| RamModule {
            part: part.into(),
            configured,
            smbios_type: t,
        };
        assert_eq!(
            ram_check(&[m("CMK32GX4M2Z3600C18", 2133, 26)])["status"],
            "warning"
        );
        assert_eq!(
            ram_check(&[m("CMK32GX4M2Z3600C18", 3600, 26)])["status"],
            "ok"
        );
        assert_eq!(
            ram_check(&[m("M378A1K43CB2-CTD", 2400, 26)])["status"],
            "info"
        );
        assert_eq!(
            ram_check(&[m("M378A1K43CB2-CTD", 3200, 26)])["status"],
            "ok"
        );
        assert_eq!(
            ram_check(&[m("M323R2GA3BB0-CQKOD", 4800, 34)])["status"],
            "info"
        );
        assert_eq!(ram_check(&[])["status"], "unknown");
    }

    #[test]
    fn refresh_check_flags_displays_below_maximum() {
        let d = |cur: u32, max: u32| Display {
            monitor: "M".into(),
            width: 1920,
            height: 1080,
            current_hz: cur,
            max_hz_same_res: max,
        };
        assert_eq!(refresh_check(&[d(239, 239)])["status"], "ok");
        assert_eq!(refresh_check(&[d(239, 240)])["status"], "ok"); // rounding, not a mistake
        assert_eq!(refresh_check(&[d(60, 144)])["status"], "warning");
        assert_eq!(
            refresh_check(&[d(240, 240), d(60, 165)])["status"],
            "warning"
        );
        assert_eq!(refresh_check(&[])["status"], "unknown");
        assert_eq!(
            parse_displays("LG ULTRAGEAR|1920|1080|239|240\nbad"),
            vec![Display {
                monitor: "LG ULTRAGEAR".into(),
                width: 1920,
                height: 1080,
                current_hz: 239,
                max_hz_same_res: 240
            }]
        );
    }

    #[test]
    fn directx_user_settings_are_parsed() {
        let s = Some("SwapEffectUpgradeEnable=1;VRROptimizeEnable=0;");
        assert_eq!(dx_flag(s, "SwapEffectUpgradeEnable"), Some(true));
        assert_eq!(dx_flag(s, "VRROptimizeEnable"), Some(false));
        assert_eq!(dx_flag(None, "SwapEffectUpgradeEnable"), None);
        let c = dx_checks(26200, s);
        assert_eq!(c[0]["status"], "ok");
        assert_eq!(c[1]["status"], "info");
        assert_eq!(dx_checks(26200, None)[0]["status"], "info");
        assert!(dx_checks(19045, s).is_empty()); // Windows 10: not available
    }

    #[test]
    fn rebar_is_read_from_nvidia_smi() {
        let out = "GPU 00000000:06:00.0\n    FB Memory Usage\n        Total                  : 16376 MiB\n        Used : 1552 MiB\n    BAR1 Memory Usage\n        Total                  : 16384 MiB\n        Used : 16355 MiB\n";
        assert_eq!(parse_nvidia_bar(out), Some((16384, 16376)));
        assert_eq!(rebar_check(true, parse_nvidia_bar(out))["status"], "ok");
        let off = out.replace("16384 MiB", "256 MiB");
        assert_eq!(rebar_check(true, parse_nvidia_bar(&off))["status"], "info");
        assert_eq!(rebar_check(false, None)["status"], "unknown");
        assert_eq!(parse_nvidia_bar("garbage"), None);
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
