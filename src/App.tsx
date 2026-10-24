import React, { useEffect, useMemo, useRef, useState } from "react";
import { check as checkUpdate } from "@tauri-apps/plugin-updater";
import { api } from "./api";
import { LangProvider, useLang, LANG_NAMES, Lang, plural, TKey } from "./i18n";
import Onboarding from "./components/Onboarding";
import { ToolErrorBoundary } from "./components/ui";
import Dashboard from "./pages/Dashboard";
import Hardware from "./pages/Hardware";
import Monitor from "./pages/Monitor";
import Latency from "./pages/Latency";
import Optimize from "./pages/Optimize";
import Cleanup from "./pages/Cleanup";
import Security from "./pages/Security";
import Benchmark from "./pages/Benchmark";
import Reports from "./pages/Reports";
import Updates from "./pages/Updates";
import Startup from "./pages/Startup";
import Processes from "./pages/Processes";
import Profiles from "./pages/Profiles";
import ScheduledTasks from "./pages/ScheduledTasks";
import RegClean from "./pages/RegClean";
import GpuTweaks from "./pages/GpuTweaks";
import NvidiaControlPanel from "./pages/NvidiaControlPanel";
import DiskAnalyzer from "./pages/DiskAnalyzer";
import BootOptimizer from "./pages/BootOptimizer";
import PrivacyCenter from "./pages/PrivacyCenter";
import ServicesManager from "./pages/ServicesManager";
import HealthCheck from "./pages/HealthCheck";
import PerfTweaks from "./pages/PerfTweaks";
import HwMonitor from "./pages/HwMonitor";
import AppUninstaller from "./pages/AppUninstaller";
import CtxMenuCleaner from "./pages/CtxMenuCleaner";
import PowerPlan from "./pages/PowerPlan";
import Debloater from "./pages/Debloater";
import DriverManager from "./pages/DriverManager";
import GameBooster from "./pages/GameBooster";
import OptimizeMyPc from "./pages/OptimizeMyPc";
import SystemCheck from "./pages/SystemCheck";
import RestorePointManager from "./pages/RestorePointManager";
import GameProfiles from "./pages/GameProfiles";
import SoftwareInstaller from "./pages/SoftwareInstaller";
import PcConfigurator from "./pages/PcConfigurator";
import Settings from "./pages/Settings";
import Changelog, { CURRENT_VERSION } from "./pages/Changelog";
import { unlockBroker, useBroker } from "./hooks/useBroker";

/**
 * `basic: true` marks a tool as safe and self-explanatory enough for Beginner
 * mode. Everything else is only reachable in Expert mode. `keywords` widen the
 * search so people find tools by what they want, not by our wording. `desc` is a
 * one-line explainer shown on the category grid tile.
 */
export type NavItem = { id: string; icon: string; label: string; desc?: string; basic?: boolean; keywords?: string };
export type NavGroup = { id: string; group: string; icon: string; desc: string; accent: string; items: NavItem[] };
type NavBuilder = (t: (key: TKey) => string) => NavGroup[];

export const buildNav: NavBuilder = (t) => [
  { id: "home", group: t("navSecDashboard"), icon: "🏠", desc: t("secDescDashboard"), accent: "blue", items: [
    { id: "dashboard", icon: "📋", label: t("navDashboard"), desc: t("toolDescDashboard"), basic: true, keywords: "home start overview status" },
    { id: "systemcheck", icon: "🔎", label: t("navSystemCheck"), desc: t("toolDescSystemCheck"), basic: true, keywords: "health check scan system diagnose recommendations empfehlungen zustand hardware" },
    { id: "healthcheck", icon: "🩺", label: t("navHealthCheck"), desc: t("toolDescHealthCheck"), basic: true, keywords: "sfc dism repair reparatur systemdateien" },
  ] },
  { id: "optimize", group: t("navSecOptimize"), icon: "🚀", desc: t("secDescOptimize"), accent: "violet", items: [
    { id: "autoopt", icon: "⚡", label: t("navAutoOpt"), desc: t("toolDescAutoOpt"), basic: true, keywords: "auto automatic oneclick one-click recommended empfohlen automatisch" },
    { id: "optimize", icon: "🎚️", label: t("navOptimize"), desc: t("toolDescOptimize"), basic: true, keywords: "tweaks fps speed schneller performance" },
    { id: "gameboost", icon: "🎮", label: t("navGameBoost"), desc: t("toolDescGameBoost"), basic: true, keywords: "gaming fps games spiele boost" },
    { id: "powerplan", icon: "🔌", label: t("navPowerPlan"), desc: t("toolDescPowerPlan"), basic: true, keywords: "power energy energie plan cpu" },
    { id: "gputweaks", icon: "🖼️", label: t("navGpuTweaks"), desc: t("toolDescGpuTweaks"), basic: true, keywords: "gpu graphics grafik nvidia amd" },
    { id: "perftweaks", icon: "⚙️", label: t("navPerfTweaks"), desc: t("toolDescPerfTweaks"), basic: true, keywords: "advanced tweaks timer mmcss" },
    { id: "latency", icon: "📉", label: t("navLatency"), desc: t("toolDescLatency"), keywords: "latency dpc ping input lag" },
    { id: "nvcontrol", icon: "🎛️", label: t("navNvControl"), desc: t("toolDescNvControl"), keywords: "nvidia driver control panel" },
    { id: "bootopt", icon: "🥾", label: t("navBootOpt"), desc: t("toolDescBootOpt"), keywords: "boot startup time bootzeit" },
  ] },
  { id: "profiles", group: t("navSecProfiles"), icon: "📁", desc: t("secDescProfiles"), accent: "violet", items: [
    { id: "profiles", icon: "📁", label: t("navProfiles"), desc: t("toolDescProfiles"), keywords: "profile preset" },
    { id: "gameprofiles", icon: "🕹️", label: t("navGameProfiles"), desc: t("toolDescGameProfiles"), keywords: "game profile per-game" },
  ] },
  { id: "cleanup", group: t("navSecCleanup"), icon: "🧹", desc: t("secDescCleanup"), accent: "cyan", items: [
    { id: "cleanup", icon: "🧼", label: t("navCleanup"), desc: t("toolDescCleanup"), basic: true, keywords: "clean temp cache junk speicher aufräumen" },
    { id: "uninstaller", icon: "🗑️", label: t("navUninstaller"), desc: t("toolDescUninstaller"), basic: true, keywords: "uninstall remove apps programme deinstallieren" },
    { id: "debloater", icon: "📦", label: t("navDebloater"), desc: t("toolDescDebloater"), basic: true, keywords: "bloat debloat remove windows apps" },
    { id: "diskanalyzer", icon: "💾", label: t("navDiskAnalyzer"), desc: t("toolDescDiskAnalyzer"), keywords: "disk space storage speicherplatz" },
    { id: "regclean", icon: "🗂️", label: t("navRegClean"), desc: t("toolDescRegClean"), keywords: "registry regedit" },
    { id: "ctxmenu", icon: "🖱️", label: t("navCtxMenu"), desc: t("toolDescCtxMenu"), keywords: "context menu rechtsklick explorer" },
  ] },
  { id: "system", group: t("navSecSystem"), icon: "🔧", desc: t("secDescSystem"), accent: "amber", items: [
    { id: "startup", icon: "🚦", label: t("navStartup"), desc: t("toolDescStartup"), basic: true, keywords: "startup autostart boot" },
    { id: "services", icon: "🔧", label: t("navServices"), desc: t("toolDescServices"), basic: true, keywords: "services dienste" },
    { id: "schedtasks", icon: "⏰", label: t("navSchedTasks"), desc: t("toolDescSchedTasks"), basic: true, keywords: "scheduled tasks aufgaben" },
    { id: "processes", icon: "📑", label: t("navProcesses"), desc: t("toolDescProcesses"), keywords: "processes tasks task manager prozesse" },
    { id: "drivers", icon: "💿", label: t("navDrivers"), desc: t("toolDescDrivers"), keywords: "drivers treiber" },
    { id: "softinstaller", icon: "📥", label: t("navSoftInstaller"), desc: t("toolDescSoftInstaller"), keywords: "install software apps winget" },
  ] },
  { id: "privacy", group: t("navSecPrivacy"), icon: "🛡️", desc: t("secDescPrivacy"), accent: "green", items: [
    { id: "privacy", icon: "🕵️", label: t("navPrivacy"), desc: t("toolDescPrivacy"), basic: true, keywords: "privacy telemetry tracking datenschutz" },
    { id: "security", icon: "🔒", label: t("navSecurity"), desc: t("toolDescSecurity"), basic: true, keywords: "security defender firewall antivirus sicherheit" },
  ] },
  { id: "monitor", group: t("navSecMonitor"), icon: "📈", desc: t("secDescMonitor"), accent: "slate", items: [
    { id: "hardware", icon: "🧩", label: t("navHardware"), desc: t("toolDescHardware"), basic: true, keywords: "hardware specs cpu gpu ram komponenten" },
    { id: "monitor", icon: "📈", label: t("navMonitor"), desc: t("toolDescMonitor"), basic: true, keywords: "monitor live metrics usage auslastung" },
    { id: "hwmonitor", icon: "🌡️", label: t("navHwMonitor"), desc: t("toolDescHwMonitor"), keywords: "sensors temps temperature lüfter fans" },
    { id: "benchmark", icon: "🏁", label: t("navBenchmark"), desc: t("toolDescBenchmark"), basic: true, keywords: "benchmark test score messen" },
    { id: "pcconfig", icon: "🛠️", label: t("navPcConfig"), desc: t("toolDescPcConfig"), keywords: "pc build configurator upgrade bottleneck" },
  ] },
  { id: "recovery", group: t("navSecRecovery"), icon: "🛟", desc: t("secDescRecovery"), accent: "green", items: [
    { id: "restorepoints", icon: "🛟", label: t("navRestorePoints"), desc: t("toolDescRestorePoints"), basic: true, keywords: "restore backup undo rückgängig wiederherstellen safety" },
    { id: "reports", icon: "📄", label: t("navReports"), desc: t("toolDescReports"), basic: true, keywords: "reports export log journal berichte" },
    { id: "updates", icon: "🔄", label: t("navUpdates"), desc: t("toolDescUpdates"), basic: true, keywords: "update upgrade version aktualisieren" },
    { id: "settings", icon: "⚙️", label: t("navSettings"), desc: t("toolDescSettings"), basic: true, keywords: "settings einstellungen options options sprache language" },
    { id: "changelog", icon: "📜", label: t("navChangelog"), desc: t("toolDescChangelog"), basic: true, keywords: "changelog changes release notes version update neu neuigkeiten versionsverlauf" },
  ] },
];

export type Mode = "beginner" | "expert";

/** Tools whose changes go through a journal/backup and can be undone, only these
 * get the "revertible" trust pill (a read-only page must not claim it), which
 * also links to the undo hub. */
const REVERTIBLE_TOOLS = new Set([
  "optimize", "autoopt", "systemcheck", "regclean", "gputweaks", "nvcontrol",
  "bootopt", "privacy", "services", "startup", "schedtasks",
  "perftweaks", "powerplan", "gameboost", "restorepoints",
  "profiles", "gameprofiles", "ctxmenu",
]);
/** Tools whose actions delete, uninstall, move or install things and can't be
 * undone from inside the app; they get a warning pill linking to restore points. */
const IRREVERSIBLE_TOOLS = new Set([
  "cleanup", "uninstaller", "diskanalyzer", "debloater", "softinstaller", "drivers", "updates",
  // SFC/DISM repairs replace system files; DISM component cleanup removes
  // superseded updates for good (they can no longer be uninstalled).
  "healthcheck",
]);

const MODE_KEY = "ui.mode";
const ONBOARDED_KEY = "ui.onboarded";
const RECENTS_KEY = "ui.recents";

/** The open tool (its section is derived from the nav catalog), plus an
 * optional feature id to deep-link and highlight inside that tool. */
type Route = { tool: string; target?: string };

/** Quick-access shortcuts on the dashboard, in the order of a sensible first pass. */
const QUICK_TOOLS = ["autoopt", "optimize", "powerplan", "gputweaks", "privacy", "startup", "cleanup", "restorepoints"] as const;

/** `helper`: the admin helper (broker) is connected. Only pages whose admin
 * actions are routed through it treat that as admin rights. */
function renderTool(page: string, mode: Mode, admin: boolean | null, go: (id: string, target?: string) => void, setMode: (m: Mode) => void, target?: string, helper = false) {
  const routed = !!admin || helper;
  switch (page) {
    case "dashboard": return <Dashboard mode={mode} go={go} />;
    case "hardware": return <Hardware mode={mode} />;
    case "monitor": return <Monitor />;
    case "latency": return <Latency />;
    case "processes": return <Processes />;
    case "startup": return <Startup admin={!!admin} focusId={target} />;
    case "schedtasks": return <ScheduledTasks admin={!!admin} focusId={target} />;
    case "optimize": return <Optimize mode={mode} admin={routed} focusId={target} onSwitchExpert={() => setMode("expert")} />;
    case "profiles": return <Profiles admin={routed} />;
    case "gameprofiles": return <GameProfiles />;
    case "cleanup": return <Cleanup admin={!!admin} />;
    case "regclean": return <RegClean admin={!!admin} />;
    case "gputweaks": return <GpuTweaks admin={!!admin} focusId={target} />;
    case "nvcontrol": return <NvidiaControlPanel admin={!!admin} />;
    case "diskanalyzer": return <DiskAnalyzer />;
    case "bootopt": return <BootOptimizer admin={!!admin} />;
    case "privacy": return <PrivacyCenter admin={routed} focusId={target} />;
    case "services": return <ServicesManager admin={!!admin} focusId={target} />;
    case "healthcheck": return <HealthCheck admin={!!admin} />;
    case "updates": return <Updates admin={!!admin} />;
    case "perftweaks": return <PerfTweaks admin={!!admin} />;
    case "hwmonitor": return <HwMonitor />;
    case "powerplan": return <PowerPlan admin={!!admin} />;
    case "uninstaller": return <AppUninstaller admin={!!admin} />;
    case "ctxmenu": return <CtxMenuCleaner admin={!!admin} />;
    case "debloater": return <Debloater admin={routed} focusId={target} />;
    case "drivers": return <DriverManager admin={!!admin} />;
    case "gameboost": return <GameBooster admin={!!admin} />;
    case "security": return <Security mode={mode} admin={!!admin} go={go} />;
    case "benchmark": return <Benchmark />;
    case "reports": return <Reports />;
    case "autoopt": return <OptimizeMyPc admin={routed} go={go} />;
    case "systemcheck": return <SystemCheck mode={mode} go={go} />;
    case "restorepoints": return <RestorePointManager admin={!!admin} />;
    case "softinstaller": return <SoftwareInstaller admin={!!admin} />;
    case "pcconfig": return <PcConfigurator />;
    case "changelog": return <Changelog />;
    case "settings": return <Settings mode={mode} setMode={setMode} onOpenChangelog={() => go("changelog")} />;
    default: return null;
  }
}

function AppInner() {
  const { lang, setLang, t } = useLang();
  const nav = buildNav(t);
  const [route, setRoute] = useState<Route>({ tool: "dashboard" });
  const [admin, setAdmin] = useState<boolean | null>(null);
  const broker = useBroker();
  const [unlocking, setUnlocking] = useState(false);
  const [brokerErr, setBrokerErr] = useState("");
  const [mode, setModeState] = useState<Mode>(() => {
    try { return (localStorage.getItem(MODE_KEY) as Mode) ?? "beginner"; } catch { return "beginner"; }
  });
  const [onboarded, setOnboarded] = useState(() => {
    try { return localStorage.getItem(ONBOARDED_KEY) === "1"; } catch { return true; }
  });
  const [query, setQuery] = useState("");
  const [recents, setRecents] = useState<string[]>(() => {
    try {
      const parsed = JSON.parse(localStorage.getItem(RECENTS_KEY) || "[]");
      return Array.isArray(parsed) ? parsed.filter((x) => typeof x === "string") : [];
    } catch { return []; }
  });
  const [featureIndex, setFeatureIndex] = useState<any[]>([]);
  const featureLoaded = useRef(false);
  const [updateBanner, setUpdateBanner] = useState<{ version: string } | null>(null);
  const [modeNotice, setModeNotice] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);
  const updateChecked = useRef(false);

  const setMode = (next: Mode) => {
    setModeState(next);
    try { localStorage.setItem(MODE_KEY, next); } catch {}
  };

  const finishOnboarding = (chosen: Mode) => {
    setMode(chosen);
    setOnboarded(true);
    try { localStorage.setItem(ONBOARDED_KEY, "1"); } catch {}
  };

  useEffect(() => { api.isAdmin().then(setAdmin).catch(() => setAdmin(false)); }, []);
  useEffect(() => {
    if (updateChecked.current) return;
    updateChecked.current = true;
    const timer = setTimeout(async () => {
      try { const update = await checkUpdate(); if (update) setUpdateBanner({ version: update.version }); } catch { /* offline or development build */ }
    }, 3000);
    return () => clearTimeout(timer);
  }, []);

  const allowedByMode = (item: NavItem) => mode === "expert" || !!item.basic;

  // Section → visible tools for the current mode. Sections with nothing to show
  // (all-advanced sections in Beginner mode) drop out of the sidebar.
  const visibleGroups = useMemo(
    () => nav.map((g) => ({ ...g, items: g.items.filter(allowedByMode) })).filter((g) => g.items.length > 0),
    [nav, mode]
  );
  const hiddenCount = mode === "beginner" ? nav.flatMap((g) => g.items).filter((i) => !i.basic).length : 0;

  const findGroupOf = (toolId: string) => nav.find((g) => g.items.some((i) => i.id === toolId));
  const findTool = (toolId: string) => nav.flatMap((g) => g.items).find((i) => i.id === toolId);

  const openTool = (id: string, target?: string) => {
    setQuery("");
    setRoute({ tool: id, target });
    // "Recently used", prepend, dedupe, cap at 6, persist.
    setRecents((prev) => {
      const next = [id, ...prev.filter((r) => r !== id)].slice(0, 6);
      try { localStorage.setItem(RECENTS_KEY, JSON.stringify(next)); } catch {}
      return next;
    });
  };
  // A section opens on its first tool the current mode allows.
  const openSection = (id: string) => {
    const first = visibleGroups.find((g) => g.id === id)?.items[0];
    if (first) openTool(first.id);
  };
  const goHome = () => openTool("dashboard");

  // Search results across every mode-allowed tool. Matches label + keywords.
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const searchResults = normalizedQuery
    ? nav.flatMap((g) => g.items.filter(allowedByMode).filter((i) =>
        i.label.toLocaleLowerCase().includes(normalizedQuery) || (i.keywords ?? "").includes(normalizedQuery)
      ).map((i) => ({ ...i, groupLabel: g.group, feature: false })))
    : [];

  // Lazy-load a searchable index of every toggleable feature across tools
  // (Optimize tweaks, GPU, Privacy, Debloater, services, startup, scheduled
  // tasks) so a search like "HAGS" can deep-link straight to the feature.
  const loadFeatures = async () => {
    if (featureLoaded.current) return;
    featureLoaded.current = true;
    const acc: { tool: string; id: string; name: string; desc: string; search: string }[] = [];
    const add = (tool: string, arr: any[] | undefined, nameK: string[], descK: string[], idK: string[]) => {
      (Array.isArray(arr) ? arr : []).forEach((it) => {
        if (!it || typeof it !== "object") return;
        const name = nameK.map((k) => it[k]).find((v) => typeof v === "string" && v);
        if (!name) return;
        const nameVariants = nameK.map((k) => it[k]).filter((v) => typeof v === "string" && v);
        const desc = descK.map((k) => it[k]).find((v) => typeof v === "string" && v) || "";
        acc.push({
          tool,
          id: idK.map((k) => it[k]).find((v) => typeof v === "string" && v) || "",
          name,
          desc,
          // Match against every name variant (e.g. both service name and
          // display name) plus the description.
          search: [...nameVariants, desc].join(" ").toLocaleLowerCase(),
        });
      });
    };
    const call = async (fn: () => Promise<any>, h: (v: any) => void) => { try { h(await fn()); } catch {} };
    await Promise.all([
      call(api.listTweaks, (v) => add("optimize", (Array.isArray(v) ? v : []).filter((x: any) => !x?.retired), ["name"], ["description", "rationale"], ["id"])),
      call(api.gpuScan, (v) => add("gputweaks", v?.tweaks, ["name"], ["description"], ["id"])),
      call(api.privacyScan, (v) => add("privacy", v?.tweaks, ["name"], ["description"], ["id"])),
      call(api.debloaterTweaksList, (v) => add("debloater", v?.tweaks, ["name"], ["desc", "description"], ["id"])),
      call(api.servicesList, (v) => add("services", v?.services, ["displayName", "name"], ["description"], ["name"])),
      call(api.startupList, (v) => add("startup", v?.items, ["name"], [""], ["name"])),
      call(api.schedTasksList, (v) => add("schedtasks", v?.tasks, ["name", "TaskName"], ["reason", "description"], ["name"])),
    ]);
    setFeatureIndex(acc);
  };
  useEffect(() => {
    if (normalizedQuery) loadFeatures();
  }, [normalizedQuery]);

  // Feature hits (deep-linkable). Each becomes a distinct result card.
  // In Beginner mode only surface features from tools that are beginner-allowed.
  const featureResults = normalizedQuery && featureIndex.length
    ? featureIndex
        .filter((f) => f.search.includes(normalizedQuery))
        .filter((f) => mode === "expert" || !!findTool(f.tool)?.basic)
        .map((f) => ({ ...f, feature: true }))
    : [];
  const allResults = [...searchResults, ...featureResults];

  // Leaving Expert mode while sitting on an advanced tool/category would strand
  // the user on a screen they can no longer reach, bounce home WITH a notice
  // instead of a silent context loss (UI-020).
  useEffect(() => {
    if (mode !== "beginner") return;
    if (!findTool(route.tool)?.basic) {
      setModeNotice(true);
      setRoute({ tool: "dashboard" });
    }
  }, [mode]);

  // Keyboard shortcuts: "/" focuses the tool search, Esc clears it.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const typing = target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
      if (e.key === "/" && !typing && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        searchRef.current?.focus();
      } else if (e.key === "Escape" && query && !typing) {
        setQuery("");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [query]);

  if (!onboarded) return <Onboarding onDone={finishOnboarding} />;

  const activeTool = findTool(route.tool);
  const activeSection = visibleGroups.find((g) => g.items.some((i) => i.id === route.tool));
  const quickTools = QUICK_TOOLS
    .map((id) => findTool(id))
    .filter((item): item is NavItem => item !== undefined && allowedByMode(item));
  const recentTools = recents
    .map((id) => findTool(id))
    .filter((item): item is NavItem => item !== undefined && allowedByMode(item) && item.id !== "dashboard" && !quickTools.includes(item));

  const toolCard = (item: NavItem, subtitle?: string) => (
    <button key={item.id} className="tool-card" onClick={() => openTool(item.id)}>
      <span className="tool-icon" aria-hidden="true">{item.icon}</span>
      <b>{item.label}</b>
      {subtitle && <span className="tool-cat">{subtitle}</span>}
      {item.desc && <span className="tool-desc">{item.desc}</span>}
    </button>
  );

  return (
    <div className="shell">
      <header className="topbar">
        <div className="topbar-left">
          <button className="brand-home" onClick={goHome} title={t("navDashboard")}>
            <span className="brand-mark">AD</span>
            <span className="logo">Hyper<span>Optimize</span></span>
          </button>
        </div>

        <div className="topbar-search">
          <span aria-hidden="true">⌕</span>
          <input ref={searchRef} value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("navSearchTools")} aria-label={t("navSearchTools")} />
          {query && <button className="topbar-search-clear" onClick={() => setQuery("")} aria-label={t("navSearchClear")}>×</button>}
        </div>

        <div className="topbar-right">
          <div className="mode-toggle" role="group" aria-label={t("navModeLabel")}>
            <button className={mode === "beginner" ? "on" : ""} aria-pressed={mode === "beginner"} onClick={() => setMode("beginner")}>{t("beginner")}</button>
            <button className={mode === "expert" ? "on" : ""} aria-pressed={mode === "expert"} onClick={() => setMode("expert")}>{t("expert")}</button>
          </div>
          <span className="admin-chip" title={admin ? t("adminBadge") : t("adminHint")}><span className={`status-dot ${admin ? "is-ready" : ""}`} />{admin === null ? "…" : admin ? t("adminBadge") : t("userBadge")}</span>
          {admin === false && broker.available && (broker.connected
            ? <span className="admin-chip" title={t("brokerActiveHint")}><span className="status-dot is-ready" />{t("brokerActive")}</span>
            : <button className="btn small" disabled={unlocking} title={t("brokerUnlockHint")}
                onClick={async () => { setUnlocking(true); setBrokerErr(""); try { await unlockBroker(); } catch (e) { setBrokerErr(String(e)); } finally { setUnlocking(false); } }}>
                {unlocking ? "…" : t("brokerUnlock")}
              </button>)}
          {brokerErr && <span className="muted" style={{ color: "var(--red)", fontSize: 11 }} title={brokerErr}>{t("brokerUnlockFailed")}</span>}
        </div>
      </header>

      {updateBanner && <div className="update-banner"><div><b>↻ {t("navUpdateAvailable")}</b><span>v{updateBanner.version} {t("navUpdateReady")}</span></div><button className="btn small" onClick={() => { openTool("updates"); setUpdateBanner(null); }}>{t("navInstallUpdate")}</button><button className="icon-button" onClick={() => setUpdateBanner(null)} aria-label="Dismiss update">×</button></div>}

      {modeNotice && (
        <div className="warn-banner" style={{ margin: "12px 0 0" }}>
          <b>ℹ </b>{t("modeNotice")}
          <button className="btn small" style={{ marginLeft: 10 }} onClick={() => { setMode("expert"); setModeNotice(false); }}>{t("modeSwitchExpert")}</button>
          <button className="btn small ghost" style={{ marginLeft: 6 }} onClick={() => setModeNotice(false)}>{t("close")}</button>
        </div>
      )}

      <div className="shell-body">
        <aside className="sidebar">
          <nav className="side-nav" aria-label={t("navMainLabel")}>
            {visibleGroups.map((g) => {
              const current = !normalizedQuery && activeSection?.id === g.id;
              return (
                <button
                  key={g.id}
                  className={`side-item ${current ? "active" : ""}`}
                  aria-current={current ? "page" : undefined}
                  onClick={() => openSection(g.id)}
                  title={g.desc}
                >
                  <span className="side-icon" aria-hidden="true">{g.icon}</span>
                  <span className="side-label">{g.group}</span>
                </button>
              );
            })}
          </nav>
          {hiddenCount > 0 && (
            <button className="side-unlock" onClick={() => setMode("expert")} title={t("modeSwitchExpert")}>
              <span aria-hidden="true">＋</span>
              <span className="side-label">{hiddenCount} {plural(hiddenCount, t("navMoreInExpertOne"), t("navMoreInExpert"))}</span>
            </button>
          )}
          <div className="side-footer">
            <select className="lang-select" value={lang} onChange={(e) => setLang(e.target.value as Lang)} aria-label="Language">
              {Object.entries(LANG_NAMES).map(([code, name]) => <option key={code} value={code}>{name}</option>)}
            </select>
            <div className="side-social">
              <button className="social-btn coffee" aria-label={t("supportTip")} onClick={() => api.openPath("https://www.buymeacoffee.com/zCrxticxl")} title={t("supportTip")}><span aria-hidden="true">☕</span></button>
              <button className="social-btn" aria-label="Discord" onClick={() => api.openPath("https://discord.gg/vFaKsVuxKP")} title="Discord"><span aria-hidden="true">D</span></button>
              <button className="social-btn x" aria-label="X" onClick={() => api.openPath("https://x.com/zCrxticxl")} title="X"><span aria-hidden="true">X</span></button>
            </div>
          </div>
        </aside>

        <main className="content">
          {/* Search overrides whatever tool is open. */}
          {normalizedQuery ? (
            <>
              <div className="view-head"><h1>{t("navSearchResults")}</h1><p>{allResults.length} · “{query}”</p></div>
              {allResults.length === 0 ? <div className="nav-empty">{t("navNoMatch")}</div> : (
                <div className="tool-grid">
                  {allResults.map((item, idx) => (
                    item.feature ? (
                      <button
                        key={`feat-${item.tool}-${item.id || idx}`}
                        className="tool-card"
                        onClick={() => openTool(item.tool, item.id || undefined)}
                        title={item.desc}
                      >
                        <span className="tool-icon" aria-hidden="true">⚙️</span>
                        <b>{item.name}</b>
                        <span className="tool-cat">→ {findTool(item.tool)?.label ?? item.tool}</span>
                        {item.desc && <span className="tool-desc">{item.desc}</span>}
                      </button>
                    ) : toolCard(item, item.groupLabel)
                  ))}
                </div>
              )}
            </>
          ) : activeSection && activeTool ? (
            <>
              <div className="section-head">
                <div className="section-title">
                  <h1>{activeSection.group}</h1>
                  <p>{activeSection.desc}</p>
                </div>
                {REVERTIBLE_TOOLS.has(route.tool) && (
                  <button className="safe-pill" style={{ border: 0, cursor: "pointer" }} onClick={() => openTool("reports")} title={t("safeRevertible")}>
                    <span className="status-dot is-ready" /><span aria-hidden="true">🛟</span> {t("safeRevertible")}
                  </button>
                )}
                {IRREVERSIBLE_TOOLS.has(route.tool) && (
                  <button className="safe-pill is-warn" style={{ border: 0, cursor: "pointer" }} onClick={() => openTool("restorepoints")} title={t("safeIrreversible")}>
                    <span className="status-dot is-warn" /><span aria-hidden="true">⚠</span> {t("safeIrreversible")}
                  </button>
                )}
              </div>
              {activeSection.items.length > 1 && (
                <div className="section-tabs" role="tablist" aria-label={activeSection.group}>
                  {activeSection.items.map((item) => (
                    <button
                      key={item.id}
                      role="tab"
                      aria-selected={item.id === route.tool}
                      className={`section-tab ${item.id === route.tool ? "active" : ""}`}
                      onClick={() => openTool(item.id)}
                      title={item.desc}
                    >
                      <span aria-hidden="true">{item.icon}</span> {item.label}
                    </button>
                  ))}
                </div>
              )}
              <div className="page-content" role="tabpanel" key={`${route.tool}:${route.target ?? ""}`}><ToolErrorBoundary resetKey={route.tool}>{renderTool(route.tool, mode, admin, openTool, setMode, route.target, broker.connected)}</ToolErrorBoundary></div>

              {route.tool === "dashboard" && (
                <section className="home-quick" aria-labelledby="home-quick-title">
                  <div className="home-section-head"><h2 id="home-quick-title">{t("homeQuickAccess")}</h2></div>
                  <div className="tool-grid">{quickTools.map((item) => toolCard(item, findGroupOf(item.id)?.group))}</div>
                  {recentTools.length > 0 && <>
                    <div className="home-section-head" style={{ marginTop: 20 }}><h2>{t("homeRecents")}</h2></div>
                    <div className="tool-grid">{recentTools.map((item) => toolCard(item, findGroupOf(item.id)?.group))}</div>
                  </>}
                  <button className="home-changelog" onClick={() => openTool("changelog")} title={t("settingsViewChangelog")}>
                    <span aria-hidden="true">✨</span>
                    <span className="home-changelog-text">{t("homeWhatsNew")} · <b>v{CURRENT_VERSION}</b></span>
                    <span className="home-changelog-cta">{t("settingsViewChangelog")} →</span>
                  </button>
                </section>
              )}
            </>
          ) : null}
        </main>
      </div>
    </div>
  );
}

export default function App() { return <LangProvider><AppInner /></LangProvider>; }
