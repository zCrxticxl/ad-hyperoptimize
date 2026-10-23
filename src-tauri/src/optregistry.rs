//! Central optimization registry: what each catalog tweak is worth, when it is
//! applicable, and what it conflicts with. Pure data + rules, no system
//! access, so every decision the health check makes is unit-testable.
//!
//! Evidence classes describe the *performance* claim, judged by what the
//! setting actually changes in Windows (see docs/audit-2026-10/
//! optimization-audit.md for the per-tweak reasoning). A privacy or comfort
//! tweak can be perfectly legitimate and still be `NoProvenBenefit` for
//! performance; its `purpose` says what it is really for.
//!
//! Nothing here was measured on real hardware. "ProvenBeneficial" is therefore
//! deliberately unused: no catalog tweak has measured, reproducible evidence of
//! an FPS gain, and the class must not be handed out on reputation alone.

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Evidence {
    /// Reliable evidence of a measurable improvement (currently no entry).
    #[allow(dead_code)]
    ProvenBeneficial,
    /// Documented mechanism that helps only under specific conditions.
    ConditionallyBeneficial,
    /// Legitimate maintenance / repair / troubleshooting; not an FPS tweak.
    SystemMaintenance,
    /// Executes correctly, but no reliable evidence of a meaningful gain.
    NoProvenBenefit,
    /// No longer applies to supported Windows versions.
    Outdated,
    /// Implementation does not do what it claims.
    #[allow(dead_code)]
    Broken,
    /// Can reduce performance, stability or security on common systems.
    PotentiallyHarmful,
    /// Effect not established.
    Unverified,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Purpose {
    Performance,
    Input,
    Repair,
    Maintenance,
    Troubleshooting,
    Privacy,
    Comfort,
}

/// Where the Optimize view shows an applicable tweak.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Group {
    Gaming,
    System,
    Advanced,
    /// Privacy and clutter settings: legitimate, but not performance.
    PrivacyClutter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Reboot {
    NotNeeded,
    SignOut,
    ExplorerRestart,
    Restart,
}

/// Applicability conditions, checked against detected system facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Req {
    /// No battery: desktop power/USB behaviour.
    Desktop,
    /// Windows 10 only (build < 22000).
    Windows10,
    /// The High performance scheme exists (absent on Modern Standby systems).
    HighPerfPlanAvailable,
    /// A dedicated NVIDIA/AMD/Intel Arc GPU is present.
    DedicatedGpu,
    /// No mechanical hard disk (SysMain prefetching still helps HDDs).
    NoHdd,
    /// Not an AMD dual-CCD X3D CPU (relies on core parking for V-Cache).
    NotDualCcdX3d,
    /// Not an Intel hybrid (P/E-core) CPU (relies on parking/throttling).
    NotHybridCpu,
    /// The system drive is low on free space (< 15 %).
    LowSystemDriveSpace,
}

pub struct OptMeta {
    pub id: &'static str,
    pub evidence: Evidence,
    pub purpose: Purpose,
    pub group: Group,
    /// What the user can realistically expect. Never a number.
    pub benefit: &'static str,
    /// When it can help (conditional entries) or why it is not offered.
    pub when: &'static str,
    pub requires: &'static [Req],
    /// Offered by "Optimize My PC" when not yet applied.
    pub one_click: bool,
    /// Listed under "Recommended" in Optimize when applicable and not applied.
    pub recommend: bool,
    pub reboot: Reboot,
    /// Other tweaks whose effect this one already includes or contradicts.
    pub redundant_with: &'static [&'static str],
}

/// Tweaks retired from the app: they change nothing measurable (myths,
/// Windows defaults, idle services, outdated settings) or can harm. They can
/// no longer be applied; the UI lists one only while it is still active so it
/// can be reset, after which it disappears. Privacy and look-and-feel tweaks
/// are not here: they have a real, if non-performance, effect.
pub static RETIRED: &[&str] = &[
    "mmcss_gaming",
    "mmcss_games_task",
    "priority_separation",
    "power_throttling_off",
    "fse_optimizations_off",
    "sysmain_off",
    "wsearch_manual",
    "ntfs_8dot3_off",
    "core_parking_off",
    "xbox_services_off",
    "nagle_off",
    "tcp_timed_wait_low",
    "irp_stack_high",
    "qos_bandwidth_free",
    "disable_paging_executive",
    "ntfs_last_access_off",
    "background_apps_off",
    "cortana_off",
    "remote_registry_off",
    "svc_geolocation_off",
    "svc_fax_off",
    "svc_wmp_network_off",
    "svc_maps_off",
    "svc_link_tracking_off",
    "svc_wisvc_off",
    "svc_diag_tracking_off",
];

pub fn is_retired(id: &str) -> bool {
    RETIRED.contains(&id)
}

/// Retired tweaks whose target is the Windows default anyway: "applied"
/// without an open journal entry just means Windows is at its default, so
/// there is nothing to reset.
pub static MATCHES_WINDOWS_DEFAULT: &[&str] = &[
    "remote_registry_off",
    "svc_geolocation_off",
    "xbox_services_off",
];

use Evidence::*;
use Group::*;
use Purpose::*;
use Reboot::*;

const fn m(
    id: &'static str,
    evidence: Evidence,
    purpose: Purpose,
    group: Group,
    benefit: &'static str,
    when: &'static str,
) -> OptMeta {
    OptMeta {
        id,
        evidence,
        purpose,
        group,
        benefit,
        when,
        requires: &[],
        one_click: false,
        recommend: false,
        reboot: NotNeeded,
        redundant_with: &[],
    }
}

const NO_GAIN: &str = "No reliable evidence of a performance gain.";
const PRIVACY: &str = "Reduces data collection or ads; not a performance change.";
const COMFORT: &str = "Changes how Windows looks or behaves; not a performance change.";

pub static REGISTRY: &[OptMeta] = &[
    // ── Gaming: conditional, documented mechanisms ───────────────────────────
    OptMeta {
        one_click: true,
        recommend: true,
        ..m("game_mode_on", ConditionallyBeneficial, Repair, Gaming,
            "Restores Windows Game Mode: Windows prioritises the game and Windows Update does not install drivers or show restart notifications while you play.",
            "Only relevant when Game Mode was turned off; it is on by default.")
    },
    OptMeta {
        one_click: true,
        recommend: true,
        reboot: SignOut,
        ..m("game_dvr_off", ConditionallyBeneficial, Performance, Gaming,
            "Stops Game Bar capture from recording in the background, which uses the GPU video encoder and disk while you play.",
            "Helps when background recording/capture was enabled. Turn it back on if you use Game Bar clips.")
    },
    OptMeta {
        recommend: true,
        reboot: SignOut,
        ..m("mouse_accel_off", ConditionallyBeneficial, Input, Gaming,
            "Makes cursor movement independent of mouse speed, so aim is consistent. Does not change FPS.",
            "Useful for mouse-aimed games. Changes how the desktop cursor feels, so it is never applied automatically.")
    },
    OptMeta {
        requires: &[Req::DedicatedGpu],
        reboot: Restart,
        ..m("hags_on", ConditionallyBeneficial, Performance, Gaming,
            "Lets the GPU schedule its own work. Required for DLSS Frame Generation; otherwise effects range from small gains to none or regressions depending on game and driver.",
            "Needs a GPU and driver that support it (WDDM 2.7+). Support cannot be confirmed before restart, so it is never applied automatically.")
    },
    OptMeta {
        requires: &[Req::Desktop, Req::HighPerfPlanAvailable],
        ..m("power_high_performance", ConditionallyBeneficial, Performance, Gaming,
            "Keeps CPU clocks higher at idle, which can reduce ramp-up latency. On modern CPUs Balanced already boosts quickly; gains are usually small and power use rises.",
            "Desktops only. Not offered on laptops or systems without the High performance plan (Modern Standby).")
    },
    // ── Repairs: restore Windows defaults that were changed ─────────────────
    OptMeta {
        one_click: true,
        recommend: true,
        ..m("tcp_autotuning_normal", SystemMaintenance, Repair, System,
            "Restores TCP receive-window auto-tuning to the Windows default. A disabled or restricted level limits download throughput on fast connections.",
            "Only relevant when auto-tuning was changed from Normal.")
    },
    OptMeta {
        one_click: true,
        recommend: true,
        ..m("net_rss_on", SystemMaintenance, Repair, System,
            "Restores Receive Side Scaling, which spreads network processing across CPU cores. It is on by default.",
            "Only relevant when RSS was turned off.")
    },
    OptMeta {
        requires: &[Req::Desktop, Req::LowSystemDriveSpace],
        recommend: true,
        ..m("hibernate_off", SystemMaintenance, Maintenance, System,
            "Deletes hiberfil.sys and frees disk space roughly the size of a large part of your RAM. Also disables Fast Startup.",
            "Offered only on desktops whose system drive is low on space. Not a performance change.")
    },
    // ── Troubleshooting: only when a specific problem exists ────────────────
    OptMeta {
        reboot: Restart,
        ..m("mpo_off", ConditionallyBeneficial, Troubleshooting, Advanced,
            "Can fix flickering, black screens or stutter caused by Multiplane Overlay on some driver/monitor combinations.",
            "Only if you actually see those symptoms. Reportedly ignored on newer Windows builds.")
    },
    OptMeta {
        requires: &[Req::Desktop],
        ..m("usb_suspend_off", ConditionallyBeneficial, Troubleshooting, Advanced,
            "Can fix USB devices that disconnect or lag after idle.",
            "Only for that problem; desktops only. Applies to the current power plan.")
    },
    OptMeta {
        redundant_with: &["hibernate_off"],
        ..m("fast_startup_off", SystemMaintenance, Troubleshooting, Advanced,
            "Makes shutdown a full shutdown. Helps dual-boot setups and drivers that misbehave after Fast Startup.",
            "Only for those cases. Already implied by disabling hibernation.")
    },
    // ── Advanced: executes correctly, no proven gain ────────────────────────
    OptMeta {
        reboot: Restart,
        ..m("mmcss_gaming", NoProvenBenefit, Performance, Advanced, NO_GAIN,
            "Only affects threads registered with the multimedia scheduler; most games do not register.")
    },
    OptMeta {
        reboot: Restart,
        ..m("mmcss_games_task", NoProvenBenefit, Performance, Advanced, NO_GAIN,
            "Only affects threads that register with the MMCSS \"Games\" task.")
    },
    m("priority_separation", NoProvenBenefit, Performance, Advanced, NO_GAIN,
        "0x26 is the same scheduling as the Windows client default."),
    OptMeta {
        requires: &[Req::Desktop, Req::NotHybridCpu],
        reboot: Restart,
        ..m("power_throttling_off", NoProvenBenefit, Performance, Advanced, NO_GAIN,
            "Power throttling targets background apps; a foreground game is not throttled. Harmful on laptops and hybrid CPUs.")
    },
    OptMeta {
        reboot: SignOut,
        ..m("fse_optimizations_off", Unverified, Performance, Advanced,
            "Effect not established; global flags are largely ignored for modern DX12/flip-model games.",
            "Per-game compatibility settings are the reliable control.")
    },
    m("startup_delay_off", NoProvenBenefit, Comfort, Advanced,
        "Startup apps launch sooner after sign-in. No effect on games.", COMFORT),
    OptMeta {
        reboot: SignOut,
        ..m("visual_fx_performance", NoProvenBenefit, Comfort, Advanced,
            "Turns off shell animations and shadows. No measurable gain on GPUs from the last decade.",
            COMFORT)
    },
    m("menu_delay_fast", NoProvenBenefit, Comfort, Advanced, "Menus open faster.", COMFORT),
    m("transparency_off", NoProvenBenefit, Comfort, Advanced, NO_GAIN, COMFORT),
    OptMeta {
        reboot: SignOut,
        ..m("min_animate_off", NoProvenBenefit, Comfort, Advanced, NO_GAIN, COMFORT)
    },
    m("gamebar_popups_off", NoProvenBenefit, Comfort, Advanced, "Fewer Game Bar pop-ups.", COMFORT),
    OptMeta {
        requires: &[Req::NoHdd],
        reboot: Restart,
        ..m("sysmain_off", NoProvenBenefit, Performance, Advanced, NO_GAIN,
            "Prefetching costs little on SSDs and still helps on hard disks; not offered when a hard disk is present.")
    },
    m("wsearch_manual", NoProvenBenefit, Performance, Advanced, NO_GAIN,
        "Slows or breaks Start menu, Outlook and Settings search."),
    m("ntfs_8dot3_off", NoProvenBenefit, Performance, Advanced, NO_GAIN,
        "Windows already disables short names on new non-system volumes; old installers may need them."),
    m("edge_prelaunch_off", NoProvenBenefit, Comfort, Advanced,
        "Stops Edge from preloading at sign-in.", "Edge will show \"managed by your organization\"."),
    m("svc_fax_off", NoProvenBenefit, Comfort, Advanced, NO_GAIN, "Fax is manual-start and idle when unused."),
    m("svc_wmp_network_off", NoProvenBenefit, Comfort, Advanced, NO_GAIN, "Often absent on Windows 11."),
    m("svc_maps_off", NoProvenBenefit, Comfort, Advanced, NO_GAIN, "Stops offline map updates."),
    m("svc_link_tracking_off", NoProvenBenefit, Comfort, Advanced, NO_GAIN, "Breaks repair of moved-file shortcuts on network shares."),
    m("svc_wisvc_off", NoProvenBenefit, Comfort, Advanced, NO_GAIN, "Breaks Windows Insider builds."),
    m("svc_diag_tracking_off", NoProvenBenefit, Comfort, Advanced, NO_GAIN,
        "Diagnostics Hub collector used by Visual Studio; idle otherwise."),
    // ── Privacy: legitimate, but not performance ────────────────────────────
    OptMeta {
        reboot: Restart,
        ..m("telemetry_minimal", NoProvenBenefit, Privacy, PrivacyClutter, PRIVACY, "Feedback Hub diagnostics stop working.")
    },
    m("advertising_id_off", NoProvenBenefit, Privacy, PrivacyClutter, PRIVACY, ""),
    m("content_suggestions_off", NoProvenBenefit, Privacy, PrivacyClutter, PRIVACY, ""),
    m("start_websearch_off", NoProvenBenefit, Privacy, PrivacyClutter, PRIVACY, "Also removes File Explorer search suggestions."),
    m("lockscreen_ads_off", NoProvenBenefit, Privacy, PrivacyClutter, PRIVACY, ""),
    m("activity_history_off", NoProvenBenefit, Privacy, PrivacyClutter, PRIVACY, ""),
    m("delivery_opt_off", NoProvenBenefit, Privacy, PrivacyClutter, "Stops sharing update data with other PCs.",
        "Updates for several PCs on one network may download more data."),
    m("wer_off", NoProvenBenefit, Privacy, PrivacyClutter, PRIVACY, "Crash reports for troubleshooting are no longer created."),
    OptMeta {
        reboot: ExplorerRestart,
        ..m("widgets_off", NoProvenBenefit, Comfort, PrivacyClutter, "Removes the Widgets board and its news feed.", COMFORT)
    },
    OptMeta {
        reboot: ExplorerRestart,
        ..m("search_highlights_off", NoProvenBenefit, Comfort, PrivacyClutter, "Removes search highlights.", COMFORT)
    },
    m("auto_play_off", NoProvenBenefit, Privacy, PrivacyClutter,
        "Stops media and USB drives from starting programs on insert (security hygiene).", ""),
    // ── Not offered: outdated or harmful on common hardware ─────────────────
    OptMeta {
        requires: &[Req::NotDualCcdX3d, Req::NotHybridCpu],
        ..m("core_parking_off", PotentiallyHarmful, Performance, Advanced,
            "Keeps all cores unparked.",
            "Defeats the core parking AMD dual-CCD X3D and Intel hybrid CPUs rely on to keep games on the right cores; High performance plans already unpark cores.")
    },
    m("xbox_services_off", PotentiallyHarmful, Performance, Advanced, NO_GAIN,
        "The services are already manual-start; forcing them off can break Game Pass and Xbox sign-in."),
    OptMeta {
        reboot: Restart,
        ..m("nagle_off", NoProvenBenefit, Performance, Advanced, NO_GAIN,
            "TCPNoDelay is not read by Windows; games set Nagle per socket, and most shooters (incl. Rainbow Six Siege) use UDP.")
    },
    OptMeta {
        reboot: Restart,
        ..m("tcp_timed_wait_low", NoProvenBenefit, Performance, Advanced, NO_GAIN,
            "Only matters for servers exhausting ports.")
    },
    OptMeta {
        reboot: Restart,
        ..m("irp_stack_high", NoProvenBenefit, Performance, Advanced, NO_GAIN,
            "Only affects the SMB file-sharing server.")
    },
    OptMeta {
        reboot: Restart,
        ..m("qos_bandwidth_free", NoProvenBenefit, Performance, Advanced, NO_GAIN,
            "Windows does not reserve 20 % bandwidth; this is a long-standing myth.")
    },
    m("remote_registry_off", NoProvenBenefit, Comfort, Advanced, NO_GAIN,
        "Already disabled by default on Windows 10/11 client editions."),
    m("svc_geolocation_off", NoProvenBenefit, Comfort, Advanced, NO_GAIN, "Already manual-start by default."),
    OptMeta {
        reboot: Restart,
        ..m("disable_paging_executive", Outdated, Performance, Advanced, NO_GAIN,
            "No measurable effect with modern memory management.")
    },
    OptMeta {
        reboot: Restart,
        ..m("ntfs_last_access_off", Outdated, Performance, Advanced, NO_GAIN,
            "Since Windows 10 1803 last-access updates are system-managed and already off on most volumes.")
    },
    m("cortana_off", Outdated, Privacy, PrivacyClutter, NO_GAIN, "The Cortana app was retired."),
    OptMeta {
        requires: &[Req::Windows10],
        ..m("background_apps_off", Outdated, Performance, Advanced, NO_GAIN,
            "Windows 11 removed this global setting; it only works on Windows 10.")
    },
];

pub fn meta(id: &str) -> Option<&'static OptMeta> {
    REGISTRY.iter().find(|m| m.id == id)
}

/// Facts the rules need, extracted from the system scan.
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub build: u32,
    pub has_battery: bool,
    pub high_perf_plan_available: bool,
    pub dedicated_gpu: bool,
    pub has_hdd: bool,
    pub dual_ccd_x3d: bool,
    pub hybrid_cpu: bool,
    pub system_free_pct: Option<f64>,
}

/// Why a tweak does not apply to this system, or None when it does.
pub fn ineligible_reason(meta: &OptMeta, f: &Facts) -> Option<&'static str> {
    meta.requires.iter().find_map(|r| {
        let ok = match r {
            Req::Desktop => !f.has_battery,
            Req::Windows10 => f.build > 0 && f.build < 22000,
            Req::HighPerfPlanAvailable => f.high_perf_plan_available,
            Req::DedicatedGpu => f.dedicated_gpu,
            Req::NoHdd => !f.has_hdd,
            Req::NotDualCcdX3d => !f.dual_ccd_x3d,
            Req::NotHybridCpu => !f.hybrid_cpu,
            Req::LowSystemDriveSpace => f.system_free_pct.is_some_and(|p| p < 15.0),
        };
        (!ok).then_some(match r {
            Req::Desktop => "Not for laptops or battery-powered systems.",
            Req::Windows10 => "Only applies to Windows 10.",
            Req::HighPerfPlanAvailable => {
                "This system has no High performance power plan (Modern Standby)."
            }
            Req::DedicatedGpu => "No dedicated GPU detected.",
            Req::NoHdd => "A hard disk is present; prefetching still helps it.",
            Req::NotDualCcdX3d => "Not suitable for AMD dual-CCD X3D CPUs.",
            Req::NotHybridCpu => "Not suitable for CPUs with performance and efficiency cores.",
            Req::LowSystemDriveSpace => "The system drive has enough free space.",
        })
    })
}

/// Whether the class may ever be offered as an improvement.
pub fn offerable(e: Evidence) -> bool {
    matches!(
        e,
        ProvenBeneficial | ConditionallyBeneficial | SystemMaintenance
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_tweak_has_exactly_one_entry() {
        let catalog: Vec<&str> = crate::tweaks::catalog_ids();
        for id in &catalog {
            assert_eq!(
                REGISTRY.iter().filter(|m| m.id == *id).count(),
                1,
                "registry entry for {id}"
            );
        }
        for m in REGISTRY {
            assert!(
                catalog.contains(&m.id),
                "registry entry {} has no tweak",
                m.id
            );
        }
    }

    #[test]
    fn one_click_only_offers_offerable_tweaks() {
        for m in REGISTRY.iter().filter(|m| m.one_click || m.recommend) {
            assert!(
                offerable(m.evidence),
                "{} is recommended but {:?}",
                m.id,
                m.evidence
            );
        }
        for m in REGISTRY.iter().filter(|m| m.one_click) {
            // One click never needs a restart to take effect and never
            // changes how input feels.
            assert_ne!(m.reboot, Restart, "{}", m.id);
            assert_ne!(m.purpose, Input, "{}", m.id);
        }
    }

    #[test]
    fn harmful_outdated_and_unverified_are_never_recommended() {
        for m in REGISTRY {
            if matches!(
                m.evidence,
                PotentiallyHarmful | Outdated | Unverified | Broken | NoProvenBenefit
            ) {
                assert!(!m.one_click && !m.recommend, "{}", m.id);
            }
        }
    }

    #[test]
    fn no_entry_claims_numbers() {
        for m in REGISTRY {
            for text in [m.benefit, m.when] {
                assert!(
                    !text.contains("FPS +") && !text.contains("% faster"),
                    "{}: {text}",
                    m.id
                );
            }
        }
    }

    #[test]
    fn retired_tweaks_are_never_offered_and_exist() {
        for id in RETIRED {
            let m = meta(id).unwrap_or_else(|| panic!("retired id {id} has no registry entry"));
            assert!(!offerable(m.evidence), "{id} is retired but offerable");
            assert!(!m.one_click && !m.recommend, "{id}");
        }
        // Privacy and comfort tweaks stay available.
        for id in [
            "telemetry_minimal",
            "transparency_off",
            "startup_delay_off",
            "visual_fx_performance",
        ] {
            assert!(!is_retired(id), "{id}");
        }
    }

    #[test]
    fn eligibility_rules() {
        let desktop = Facts {
            build: 26200,
            high_perf_plan_available: true,
            dedicated_gpu: true,
            system_free_pct: Some(40.0),
            ..Default::default()
        };
        let laptop = Facts {
            has_battery: true,
            ..desktop.clone()
        };
        let pp = meta("power_high_performance").unwrap();
        assert_eq!(ineligible_reason(pp, &desktop), None);
        assert!(ineligible_reason(pp, &laptop).is_some());
        let no_plan = Facts {
            high_perf_plan_available: false,
            ..desktop.clone()
        };
        assert!(ineligible_reason(pp, &no_plan).is_some());

        let cp = meta("core_parking_off").unwrap();
        let x3d = Facts {
            dual_ccd_x3d: true,
            ..desktop.clone()
        };
        assert!(ineligible_reason(cp, &x3d).is_some());

        let bg = meta("background_apps_off").unwrap();
        assert!(ineligible_reason(bg, &desktop).is_some());
        let win10 = Facts {
            build: 19045,
            ..desktop.clone()
        };
        assert_eq!(ineligible_reason(bg, &win10), None);

        let hib = meta("hibernate_off").unwrap();
        assert!(ineligible_reason(hib, &desktop).is_some());
        let full = Facts {
            system_free_pct: Some(8.0),
            ..desktop.clone()
        };
        assert_eq!(ineligible_reason(hib, &full), None);
        // Unknown free space never qualifies.
        let unknown = Facts {
            system_free_pct: None,
            ..desktop
        };
        assert!(ineligible_reason(hib, &unknown).is_some());
    }
}
