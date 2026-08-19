import React, { useEffect, useMemo, useRef, useState } from "react";
import { check as checkUpdate } from "@tauri-apps/plugin-updater";
import { api } from "./api";
import { LangProvider, useLang, LANG_NAMES, Lang, plural } from "./i18n";
import Onboarding from "./components/Onboarding";
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
import AutoOptimizer from "./pages/AutoOptimizer";
import RestorePointManager from "./pages/RestorePointManager";
import GameProfiles from "./pages/GameProfiles";
import SoftwareInstaller from "./pages/SoftwareInstaller";
import PcConfigurator from "./pages/PcConfigurator";
import Settings from "./pages/Settings";

/**
 * `basic: true` marks a tool as safe and self-explanatory enough for Beginner
 * mode. Everything else is only reachable in Expert mode. `keywords` widen the
 * search so people find tools by what they want, not by our wording. `desc` is a
 * one-line explainer shown on the category grid tile.
 */
export type NavItem = { id: string; icon: string; label: string; desc?: string; basic?: boolean; keywords?: string };
export type NavGroup = { id: string; group: string; icon: string; desc: string; accent: string; items: NavItem[] };
type NavBuilder = (t: (key: any) => string) => NavGroup[];

export const buildNav: NavBuilder = (t) => [
  { id: "overview", group: t("navGrpOverview"), icon: "🏠", desc: t("catDescOverview"), accent: "blue", items: [
    { id: "dashboard", icon: "📋", label: t("navDashboard"), desc: t("toolDescDashboard"), basic: true, keywords: "home start overview status" },
    { id: "autoopt", icon: "⚡", label: t("navAutoOpt"), desc: t("toolDescAutoOpt"), basic: true, keywords: "auto automatic oneclick one-click recommended empfohlen automatisch" },
    { id: "healthcheck", icon: "🩺", label: t("navHealthCheck"), desc: t("toolDescHealthCheck"), basic: true, keywords: "health scan diagnose check zustand" },
  ] },
  { id: "performance", group: t("navGrpPerformance"), icon: "🚀", desc: t("catDescPerformance"), accent: "violet", items: [
    { id: "optimize", icon: "🎚️", label: t("navOptimize"), desc: t("toolDescOptimize"), basic: true, keywords: "tweaks fps speed schneller performance" },
    { id: "gameboost", icon: "🎮", label: t("navGameBoost"), desc: t("toolDescGameBoost"), basic: true, keywords: "gaming fps games spiele boost" },
    { id: "powerplan", icon: "🔌", label: t("navPowerPlan"), desc: t("toolDescPowerPlan"), basic: true, keywords: "power energy energie plan cpu" },
    { id: "perftweaks", icon: "⚙️", label: t("navPerfTweaks"), desc: t("toolDescPerfTweaks"), keywords: "advanced tweaks timer mmcss" },
    { id: "latency", icon: "📉", label: t("navLatency"), desc: t("toolDescLatency"), keywords: "latency dpc ping input lag" },
    { id: "gputweaks", icon: "🖼️", label: t("navGpuTweaks"), desc: t("toolDescGpuTweaks"), keywords: "gpu graphics grafik nvidia amd" },
    { id: "nvcontrol", icon: "🎛️", label: t("navNvControl"), desc: t("toolDescNvControl"), keywords: "nvidia driver control panel" },
    { id: "profiles", icon: "📁", label: t("navProfiles"), desc: t("toolDescProfiles"), keywords: "profile preset" },
    { id: "gameprofiles", icon: "🕹️", label: t("navGameProfiles"), desc: t("toolDescGameProfiles"), keywords: "game profile per-game" },
  ] },
  { id: "cleanup", group: t("navGrpCleanup"), icon: "🧹", desc: t("catDescCleanup"), accent: "cyan", items: [
    { id: "cleanup", icon: "🧼", label: t("navCleanup"), desc: t("toolDescCleanup"), basic: true, keywords: "clean temp cache junk speicher aufräumen" },
    { id: "uninstaller", icon: "🗑️", label: t("navUninstaller"), desc: t("toolDescUninstaller"), basic: true, keywords: "uninstall remove apps programme deinstallieren" },
    { id: "diskanalyzer", icon: "💾", label: t("navDiskAnalyzer"), desc: t("toolDescDiskAnalyzer"), keywords: "disk space storage speicherplatz" },
    { id: "debloater", icon: "📦", label: t("navDebloater"), desc: t("toolDescDebloater"), keywords: "bloat debloat remove windows apps" },
    { id: "regclean", icon: "🗂️", label: t("navRegClean"), desc: t("toolDescRegClean"), keywords: "registry regedit" },
    { id: "ctxmenu", icon: "🖱️", label: t("navCtxMenu"), desc: t("toolDescCtxMenu"), keywords: "context menu rechtsklick explorer" },
  ] },
  { id: "protection", group: t("navGrpProtection"), icon: "🛡️", desc: t("catDescProtection"), accent: "green", items: [
    { id: "restorepoints", icon: "🛟", label: t("navRestorePoints"), desc: t("toolDescRestorePoints"), basic: true, keywords: "restore backup undo rückgängig wiederherstellen safety" },
    { id: "privacy", icon: "🕵️", label: t("navPrivacy"), desc: t("toolDescPrivacy"), basic: true, keywords: "privacy telemetry tracking datenschutz" },
    { id: "security", icon: "🔒", label: t("navSecurity"), desc: t("toolDescSecurity"), basic: true, keywords: "security defender firewall antivirus sicherheit" },
  ] },
  { id: "system", group: t("navGrpSystem"), icon: "🖥️", desc: t("catDescSystem"), accent: "amber", items: [
    { id: "hardware", icon: "🧩", label: t("navHardware"), desc: t("toolDescHardware"), basic: true, keywords: "hardware specs cpu gpu ram komponenten" },
    { id: "monitor", icon: "📈", label: t("navMonitor"), desc: t("toolDescMonitor"), basic: true, keywords: "monitor live metrics usage auslastung" },
    { id: "hwmonitor", icon: "🌡️", label: t("navHwMonitor"), desc: t("toolDescHwMonitor"), keywords: "sensors temps temperature lüfter fans" },
    { id: "processes", icon: "📑", label: t("navProcesses"), desc: t("toolDescProcesses"), keywords: "processes tasks task manager prozesse" },
    { id: "startup", icon: "🚦", label: t("navStartup"), desc: t("toolDescStartup"), keywords: "startup autostart boot" },
    { id: "services", icon: "🔧", label: t("navServices"), desc: t("toolDescServices"), keywords: "services dienste" },
    { id: "schedtasks", icon: "⏰", label: t("navSchedTasks"), desc: t("toolDescSchedTasks"), keywords: "scheduled tasks aufgaben" },
    { id: "drivers", icon: "💿", label: t("navDrivers"), desc: t("toolDescDrivers"), keywords: "drivers treiber" },
    { id: "bootopt", icon: "🥾", label: t("navBootOpt"), desc: t("toolDescBootOpt"), keywords: "boot startup time bootzeit" },
    { id: "updates", icon: "🔄", label: t("navUpdates"), desc: t("toolDescUpdates"), basic: true, keywords: "update upgrade version aktualisieren" },
    { id: "softinstaller", icon: "📥", label: t("navSoftInstaller"), desc: t("toolDescSoftInstaller"), keywords: "install software apps winget" },
    { id: "pcconfig", icon: "🛠️", label: t("navPcConfig"), desc: t("toolDescPcConfig"), keywords: "pc build configurator upgrade bottleneck" },
    { id: "settings", icon: "⚙️", label: t("navSettings"), desc: t("toolDescSettings"), basic: true, keywords: "settings einstellungen options options sprache language" },
  ] },
  { id: "reports", group: t("navGrpReports"), icon: "📊", desc: t("catDescReports"), accent: "slate", items: [
    { id: "benchmark", icon: "🏁", label: t("navBenchmark"), desc: t("toolDescBenchmark"), basic: true, keywords: "benchmark test score messen" },
    { id: "reports", icon: "📄", label: t("navReports"), desc: t("toolDescReports"), keywords: "reports export log journal berichte" },
  ] },
];

export type Mode = "beginner" | "expert";

/** Tools that change system state — only these get the "revertible" trust pill
 * (a read-only page must not claim it), which now also links to the undo hub. */
const MUTATING_TOOLS = new Set([
  "optimize", "autoopt", "cleanup", "uninstaller", "regclean", "gputweaks", "nvcontrol",
  "bootopt", "privacy", "services", "startup", "schedtasks", "healthcheck", "updates",
  "perftweaks", "powerplan", "gameboost", "debloater", "drivers", "restorepoints",
  "profiles", "gameprofiles", "ctxmenu", "softinstaller", "diskanalyzer",
]);

const MODE_KEY = "ui.mode";
const ONBOARDED_KEY = "ui.onboarded";
const RECENTS_KEY = "ui.recents";

/** Where the shell currently is: launcher home, a category grid, or a tool. */
type Route =
  | { kind: "home" }
  | { kind: "category"; id: string }
  | { kind: "tool"; id: string; target?: string };

function renderTool(page: string, mode: Mode, admin: boolean | null, go: (id: string, target?: string) => void, setMode: (m: Mode) => void, target?: string) {
  switch (page) {
    case "dashboard": return <Dashboard mode={mode} go={go} />;
    case "hardware": return <Hardware mode={mode} />;
    case "monitor": return <Monitor />;
    case "latency": return <Latency />;
    case "processes": return <Processes />;
    case "startup": return <Startup admin={!!admin} />;
    case "schedtasks": return <ScheduledTasks admin={!!admin} />;
    case "optimize": return <Optimize mode={mode} admin={!!admin} focusId={target} onSwitchExpert={() => setMode("expert")} />;
    case "profiles": return <Profiles />;
    case "gameprofiles": return <GameProfiles />;
    case "cleanup": return <Cleanup />;
    case "regclean": return <RegClean admin={!!admin} />;
    case "gputweaks": return <GpuTweaks admin={!!admin} />;
    case "nvcontrol": return <NvidiaControlPanel />;
    case "diskanalyzer": return <DiskAnalyzer />;
    case "bootopt": return <BootOptimizer admin={!!admin} />;
    case "privacy": return <PrivacyCenter admin={!!admin} />;
    case "services": return <ServicesManager admin={!!admin} />;
    case "healthcheck": return <HealthCheck admin={!!admin} />;
    case "updates": return <Updates admin={!!admin} />;
    case "perftweaks": return <PerfTweaks admin={!!admin} />;
    case "hwmonitor": return <HwMonitor />;
    case "powerplan": return <PowerPlan admin={!!admin} />;
    case "uninstaller": return <AppUninstaller admin={!!admin} />;
    case "ctxmenu": return <CtxMenuCleaner admin={!!admin} />;
    case "debloater": return <Debloater admin={!!admin} />;
    case "drivers": return <DriverManager />;
    case "gameboost": return <GameBooster admin={!!admin} />;
    case "security": return <Security mode={mode} />;
    case "benchmark": return <Benchmark />;
    case "reports": return <Reports />;
    case "autoopt": return <AutoOptimizer admin={!!admin} />;
    case "restorepoints": return <RestorePointManager admin={!!admin} />;
    case "softinstaller": return <SoftwareInstaller />;
    case "pcconfig": return <PcConfigurator />;
    case "settings": return <Settings mode={mode} setMode={setMode} />;
    default: return null;
  }
}

function AppInner() {
  const { lang, setLang, t } = useLang();
  const nav = buildNav(t);
  const [route, setRoute] = useState<Route>({ kind: "home" });
  const [admin, setAdmin] = useState<boolean | null>(null);
  const [mode, setModeState] = useState<Mode>(() => {
    try { return (localStorage.getItem(MODE_KEY) as Mode) ?? "beginner"; } catch { return "beginner"; }
  });
  const [onboarded, setOnboarded] = useState(() => {
    try { return localStorage.getItem(ONBOARDED_KEY) === "1"; } catch { return true; }
  });
  const [query, setQuery] = useState("");
  const [recents, setRecents] = useState<string[]>(() => {
    try { return JSON.parse(localStorage.getItem(RECENTS_KEY) || "[]"); } catch { return []; }
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

  // Category → visible tools for the current mode. Groups with nothing to show
  // (all-advanced categories in Beginner mode) drop out of the launcher.
  const visibleGroups = useMemo(
    () => nav.map((g) => ({ ...g, items: g.items.filter(allowedByMode) })).filter((g) => g.items.length > 0),
    [nav, mode]
  );
  const hiddenCount = mode === "beginner" ? nav.flatMap((g) => g.items).filter((i) => !i.basic).length : 0;

  const findGroupOf = (toolId: string) => nav.find((g) => g.items.some((i) => i.id === toolId));
  const findTool = (toolId: string) => nav.flatMap((g) => g.items).find((i) => i.id === toolId);

  const openCategory = (id: string) => { setQuery(""); setRoute({ kind: "category", id }); };
  const openTool = (id: string, target?: string) => {
    setQuery("");
    setRoute({ kind: "tool", id, target });
    // "Recently used" — prepend, dedupe, cap at 6, persist.
    setRecents((prev) => {
      const next = [id, ...prev.filter((r) => r !== id)].slice(0, 6);
      try { localStorage.setItem(RECENTS_KEY, JSON.stringify(next)); } catch {}
      return next;
    });
  };
  const goHome = () => { setQuery(""); setRoute({ kind: "home" }); };
  const back = () => {
    if (route.kind === "tool") { const g = findGroupOf(route.id); setRoute(g ? { kind: "category", id: g.id } : { kind: "home" }); }
    else setRoute({ kind: "home" });
  };

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
    const acc: { tool: string; id: string; name: string; desc: string }[] = [];
    const add = (tool: string, arr: any[] | undefined, nameK: string[], descK: string[], idK: string[]) => {
      (Array.isArray(arr) ? arr : []).forEach((it) => {
        if (!it || typeof it !== "object") return;
        const name = nameK.map((k) => it[k]).find((v) => typeof v === "string" && v);
        if (!name) return;
        acc.push({
          tool,
          id: idK.map((k) => it[k]).find((v) => typeof v === "string" && v) || "",
          name,
          desc: descK.map((k) => it[k]).find((v) => typeof v === "string" && v) || "",
        });
      });
    };
    const call = async (fn: () => Promise<any>, h: (v: any) => void) => { try { h(await fn()); } catch {} };
    await Promise.all([
      call(api.listTweaks, (v) => add("optimize", v, ["name"], ["description", "rationale"], ["id"])),
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
  const featureResults = normalizedQuery && featureIndex.length
    ? featureIndex
        .filter((f) => (f.name + " " + f.desc).toLocaleLowerCase().includes(normalizedQuery))
        .map((f) => ({ ...f, feature: true }))
    : [];
  const allResults = [...searchResults, ...featureResults];

  // Leaving Expert mode while sitting on an advanced tool/category would strand
  // the user on a screen they can no longer reach — bounce home WITH a notice
  // instead of a silent context loss (UI-020).
  useEffect(() => {
    if (mode !== "beginner") return;
    let stranded = false;
    if (route.kind === "tool" && !findTool(route.id)?.basic) stranded = true;
    if (route.kind === "category" && !visibleGroups.some((g) => g.id === route.id)) stranded = true;
    if (stranded) {
      setModeNotice(true);
      setRoute({ kind: "home" });
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
      } else if (e.key === "Escape" && query) {
        setQuery("");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [query]);

  if (!onboarded) return <Onboarding onDone={finishOnboarding} />;

  const activeGroup = route.kind === "category" ? nav.find((g) => g.id === route.id) : undefined;
  const activeTool = route.kind === "tool" ? findTool(route.id) : undefined;
  const activeToolGroup = route.kind === "tool" ? findGroupOf(route.id) : undefined;

  return (
    <div className="shell">
      <header className="topbar">
        <div className="topbar-left">
          <button className="brand-home" onClick={goHome} title={t("navDashboard")}>
            <span className="brand-mark">AD</span>
            <span className="logo">Hyper<span>Optimize</span></span>
          </button>
          <nav className="crumbs" aria-label="Breadcrumb">
            <button className={`crumb ${route.kind === "home" ? "current" : ""}`} onClick={goHome}>{t("navHome")}</button>
            {route.kind === "category" && activeGroup && <><span className="crumb-sep">/</span><span className="crumb current">{activeGroup.group}</span></>}
            {route.kind === "tool" && activeToolGroup && <>
              <span className="crumb-sep">/</span>
              <button className="crumb" onClick={() => openCategory(activeToolGroup.id)}>{activeToolGroup.group}</button>
              <span className="crumb-sep">/</span><span className="crumb current">{activeTool?.label}</span>
            </>}
          </nav>
        </div>

        <div className="topbar-search">
          <span aria-hidden="true">⌕</span>
          <input ref={searchRef} value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("navSearchTools")} aria-label={t("navSearchTools")} />
          {query && <button className="topbar-search-clear" onClick={() => setQuery("")} aria-label="Clear search">×</button>}
        </div>

        <div className="topbar-right">
          <div className="mode-toggle">
            <button className={mode === "beginner" ? "on" : ""} onClick={() => setMode("beginner")}>{t("beginner")}</button>
            <button className={mode === "expert" ? "on" : ""} onClick={() => setMode("expert")}>{t("expert")}</button>
          </div>
          <select className="lang-select" value={lang} onChange={(e) => setLang(e.target.value as Lang)} aria-label="Language">
            {Object.entries(LANG_NAMES).map(([code, name]) => <option key={code} value={code}>{name}</option>)}
          </select>
          <span className="admin-chip" title={admin ? t("adminBadge") : t("adminHint")}><span className={`status-dot ${admin ? "is-ready" : ""}`} />{admin === null ? "…" : admin ? t("adminBadge") : t("userBadge")}</span>
          <button className="social-btn coffee" aria-label={t("supportTip")} onClick={() => api.openPath("https://www.buymeacoffee.com/zCrxticxl")} title={t("supportTip")}><span aria-hidden="true">☕</span></button>
          <button className="social-btn" aria-label="Discord" onClick={() => api.openPath("https://discord.gg/vFaKsVuxKP")} title="Discord"><span aria-hidden="true">D</span></button>
          <button className="social-btn x" aria-label="X" onClick={() => api.openPath("https://x.com/zCrxticxl")} title="X"><span aria-hidden="true">X</span></button>
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

      <main className="content">
        {/* Search overrides whatever route we're on. */}
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
                  ) : (
                    <button key={item.id} className="tool-card" onClick={() => openTool(item.id)}>
                      <span className="tool-icon" aria-hidden="true">{item.icon}</span>
                      <b>{item.label}</b>
                      <span className="tool-cat">{item.groupLabel}</span>
                      {item.desc && <span className="tool-desc">{item.desc}</span>}
                    </button>
                  )
                ))}
              </div>
            )}
          </>
        ) : route.kind === "home" ? (
          <>
            <div className="view-head hero">
              <div>
                <div className="eyebrow">{t("homeEyebrow")}</div>
                <h1>{t("homeTitle")}</h1>
                <p>{t("homeSub")}</p>
              </div>
              <button className="btn big" onClick={() => openTool("autoopt")}><span aria-hidden="true">⚡</span> {t("navAutoOpt")}</button>
            </div>

            {recents.length > 0 && (
              <>
                <div style={{ fontSize: 11, fontWeight: 700, textTransform: "uppercase", letterSpacing: ".1em", color: "var(--muted)", margin: "6px 0 10px" }}>
                  🕘 {t("homeRecents")}
                </div>
                <div className="tool-grid" style={{ marginBottom: 22 }}>
                  {recents.map((id) => {
                    const item = findTool(id);
                    if (!item || !allowedByMode(item)) return null;
                    return (
                      <button key={id} className="tool-card" onClick={() => openTool(id)}>
                        <span className="tool-icon" aria-hidden="true">{item.icon}</span>
                        <b>{item.label}</b>
                        {item.desc && <span className="tool-desc">{item.desc}</span>}
                      </button>
                    );
                  })}
                </div>
              </>
            )}

            <div className="category-grid">
              {visibleGroups.map((g) => (
                <button key={g.id} className={`category-card accent-${g.accent}`} onClick={() => openCategory(g.id)}>
                  <span className="category-icon" aria-hidden="true">{g.icon}</span>
                  <b>{g.group}</b>
                  <span className="category-desc">{g.desc}</span>
                  <span className="category-count">{g.items.length} {plural(g.items.length, t("homeTool"), t("homeTools"))} →</span>
                </button>
              ))}
            </div>
            {hiddenCount > 0 && (
              <button className="nav-unlock wide" onClick={() => setMode("expert")}>
                <span aria-hidden="true">＋</span>
                <span>{hiddenCount} {plural(hiddenCount, t("navMoreInExpertOne"), t("navMoreInExpert"))}</span>
              </button>
            )}
          </>
        ) : route.kind === "category" && activeGroup ? (
          <>
            <div className="view-head">
              <button className="back-btn" onClick={back}>← {t("navHome")}</button>
              <div className="view-head-title"><span className={`category-icon inline accent-${activeGroup.accent}`} aria-hidden="true">{activeGroup.icon}</span><div><h1>{activeGroup.group}</h1><p>{activeGroup.desc}</p></div></div>
            </div>
            <div className="tool-grid">
              {activeGroup.items.filter(allowedByMode).map((item) => (
                <button key={item.id} className="tool-card" onClick={() => openTool(item.id)}>
                  <span className="tool-icon" aria-hidden="true">{item.icon}</span>
                  <b>{item.label}</b>
                  {item.desc && <span className="tool-desc">{item.desc}</span>}
                </button>
              ))}
            </div>
          </>
        ) : route.kind === "tool" ? (
          <>
            <div className="tool-head">
              <button className="back-btn" onClick={back}>← {activeToolGroup?.group ?? t("navHome")}</button>
              {MUTATING_TOOLS.has(route.id) && (
                <button className="safe-pill" style={{ border: 0, cursor: "pointer" }} onClick={() => openTool("reports")} title={t("safeRevertible")}>
                  <span className="status-dot is-ready" /><span aria-hidden="true">🛟</span> {t("safeRevertible")}
                </button>
              )}
            </div>
            <div className="page-content">{renderTool(route.id, mode, admin, openTool, setMode, route.target)}</div>
          </>
        ) : null}
      </main>
    </div>
  );
}

export default function App() { return <LangProvider><AppInner /></LangProvider>; }
