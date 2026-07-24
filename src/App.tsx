import React, { useEffect, useMemo, useRef, useState } from "react";
import { check as checkUpdate } from "@tauri-apps/plugin-updater";
import { api } from "./api";
import { LangProvider, useLang, LANG_NAMES, Lang } from "./i18n";
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
  ] },
  { id: "reports", group: t("navGrpReports"), icon: "📊", desc: t("catDescReports"), accent: "slate", items: [
    { id: "benchmark", icon: "🏁", label: t("navBenchmark"), desc: t("toolDescBenchmark"), basic: true, keywords: "benchmark test score messen" },
    { id: "reports", icon: "📄", label: t("navReports"), desc: t("toolDescReports"), keywords: "reports export log journal berichte" },
  ] },
];

export type Mode = "beginner" | "expert";

const MODE_KEY = "ui.mode";
const ONBOARDED_KEY = "ui.onboarded";

/** Where the shell currently is: launcher home, a category grid, or a tool. */
type Route = { kind: "home" } | { kind: "category"; id: string } | { kind: "tool"; id: string };

function renderTool(page: string, mode: Mode, admin: boolean | null, go: (id: string) => void) {
  switch (page) {
    case "dashboard": return <Dashboard mode={mode} go={go} />;
    case "hardware": return <Hardware mode={mode} />;
    case "monitor": return <Monitor />;
    case "latency": return <Latency />;
    case "processes": return <Processes />;
    case "startup": return <Startup admin={!!admin} />;
    case "schedtasks": return <ScheduledTasks admin={!!admin} />;
    case "optimize": return <Optimize mode={mode} admin={!!admin} />;
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
  const [updateBanner, setUpdateBanner] = useState<{ version: string } | null>(null);
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
  const openTool = (id: string) => { setQuery(""); setRoute({ kind: "tool", id }); };
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
      ).map((i) => ({ ...i, groupLabel: g.group })))
    : [];

  // Leaving Expert mode while sitting on an advanced tool/category would strand
  // the user on a screen they can no longer reach.
  useEffect(() => {
    if (mode !== "beginner") return;
    if (route.kind === "tool" && !findTool(route.id)?.basic) setRoute({ kind: "home" });
    if (route.kind === "category" && !visibleGroups.some((g) => g.id === route.id)) setRoute({ kind: "home" });
  }, [mode]);

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
          <input value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("navSearchTools")} aria-label={t("navSearchTools")} />
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
          <span className="admin-chip" title={admin ? t("adminBadge") : t("userBadge")}><span className={`status-dot ${admin ? "is-ready" : ""}`} />{admin === null ? "…" : admin ? "Admin" : t("userBadge")}</span>
          <button className="social-btn coffee" onClick={() => api.openPath("https://www.buymeacoffee.com/zCrxticxl")} title={t("supportTip")}>☕</button>
          <button className="social-btn" onClick={() => api.openPath("https://discord.gg/vFaKsVuxKP")} title="Discord">D</button>
          <button className="social-btn x" onClick={() => api.openPath("https://x.com/zCrxticxl")} title="X">X</button>
        </div>
      </header>

      {updateBanner && <div className="update-banner"><div><b>↻ {t("navUpdateAvailable")}</b><span>v{updateBanner.version} {t("navUpdateReady")}</span></div><button className="btn small" onClick={() => { openTool("updates"); setUpdateBanner(null); }}>{t("navInstallUpdate")}</button><button className="icon-button" onClick={() => setUpdateBanner(null)} aria-label="Dismiss update">×</button></div>}

      <main className="content">
        {/* Search overrides whatever route we're on. */}
        {normalizedQuery ? (
          <>
            <div className="view-head"><h1>{t("navSearchResults")}</h1><p>{searchResults.length} · “{query}”</p></div>
            {searchResults.length === 0 ? <div className="nav-empty">{t("navNoMatch")}</div> : (
              <div className="tool-grid">
                {searchResults.map((item) => (
                  <button key={item.id} className="tool-card" onClick={() => openTool(item.id)}>
                    <span className="tool-icon" aria-hidden="true">{item.icon}</span>
                    <b>{item.label}</b>
                    <span className="tool-cat">{item.groupLabel}</span>
                    {item.desc && <span className="tool-desc">{item.desc}</span>}
                  </button>
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
              <button className="btn big" onClick={() => openTool("autoopt")}>⚡ {t("navAutoOpt")}</button>
            </div>
            <div className="category-grid">
              {visibleGroups.map((g) => (
                <button key={g.id} className={`category-card accent-${g.accent}`} onClick={() => openCategory(g.id)}>
                  <span className="category-icon" aria-hidden="true">{g.icon}</span>
                  <b>{g.group}</b>
                  <span className="category-desc">{g.desc}</span>
                  <span className="category-count">{g.items.length} {t("homeTools")} →</span>
                </button>
              ))}
            </div>
            {hiddenCount > 0 && (
              <button className="nav-unlock wide" onClick={() => setMode("expert")}>
                <span aria-hidden="true">＋</span>
                <span>{hiddenCount} {t("navMoreInExpert")}</span>
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
              <span className="safe-pill"><span className="status-dot is-ready" />{t("safeRevertible")}</span>
            </div>
            <div className="page-content">{renderTool(route.id, mode, admin, openTool)}</div>
          </>
        ) : null}
      </main>
    </div>
  );
}

export default function App() { return <LangProvider><AppInner /></LangProvider>; }
