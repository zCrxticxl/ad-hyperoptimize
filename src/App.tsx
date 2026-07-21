import React, { useEffect, useMemo, useRef, useState } from "react";
import { check as checkUpdate } from "@tauri-apps/plugin-updater";
import { api } from "./api";
import { LangProvider, useLang, LANG_NAMES, Lang } from "./i18n";
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

type NavItem = { id: string; icon: string; label: string };
type NavGroup = { group: string; items: NavItem[] };
type NavBuilder = (t: (key: any) => string) => NavGroup[];

const buildNav: NavBuilder = (t) => [
  { group: t("navGrpOverview"), items: [
    { id: "dashboard", icon: "⌂", label: t("navDashboard") }, { id: "hardware", icon: "▦", label: t("navHardware") },
    { id: "monitor", icon: "◫", label: t("navMonitor") }, { id: "hwmonitor", icon: "◌", label: t("navHwMonitor") },
    { id: "healthcheck", icon: "✓", label: t("navHealthCheck") },
  ] },
  { group: t("navGrpPerformance"), items: [
    { id: "autoopt", icon: "✦", label: t("navAutoOpt") }, { id: "perftweaks", icon: "ϟ", label: t("navPerfTweaks") },
    { id: "optimize", icon: "↗", label: t("navOptimize") }, { id: "gameboost", icon: "◈", label: t("navGameBoost") },
    { id: "powerplan", icon: "⌁", label: t("navPowerPlan") }, { id: "latency", icon: "◒", label: t("navLatency") },
    { id: "gputweaks", icon: "▣", label: t("navGpuTweaks") }, { id: "nvcontrol", icon: "●", label: t("navNvControl") },
    { id: "profiles", icon: "◎", label: t("navProfiles") }, { id: "gameprofiles", icon: "◇", label: t("navGameProfiles") },
    { id: "pcconfig", icon: "▤", label: t("navPcConfig") },
  ] },
  { group: t("navGrpCleanup"), items: [
    { id: "cleanup", icon: "◫", label: t("navCleanup") }, { id: "uninstaller", icon: "×", label: t("navUninstaller") },
    { id: "diskanalyzer", icon: "◉", label: t("navDiskAnalyzer") }, { id: "regclean", icon: "⌘", label: t("navRegClean") },
    { id: "debloater", icon: "◌", label: t("navDebloater") }, { id: "ctxmenu", icon: "☰", label: t("navCtxMenu") },
  ] },
  { group: t("navGrpPrivacySecurity"), items: [
    { id: "privacy", icon: "◈", label: t("navPrivacy") }, { id: "security", icon: "◉", label: t("navSecurity") },
  ] },
  { group: t("navGrpSystem"), items: [
    { id: "processes", icon: "≡", label: t("navProcesses") }, { id: "startup", icon: "▶", label: t("navStartup") },
    { id: "services", icon: "⚙", label: t("navServices") }, { id: "schedtasks", icon: "◷", label: t("navSchedTasks") },
    { id: "drivers", icon: "⌕", label: t("navDrivers") }, { id: "bootopt", icon: "◴", label: t("navBootOpt") },
    { id: "restorepoints", icon: "↶", label: t("navRestorePoints") }, { id: "updates", icon: "↻", label: t("navUpdates") },
    { id: "softinstaller", icon: "□", label: t("navSoftInstaller") },
  ] },
  { group: t("navGrpReports"), items: [
    { id: "benchmark", icon: "◴", label: t("navBenchmark") }, { id: "reports", icon: "▤", label: t("navReports") },
  ] },
];

export type Mode = "beginner" | "expert";

function AppInner() {
  const { lang, setLang, t } = useLang();
  const nav = buildNav(t);
  const [page, setPage] = useState("dashboard");
  const [admin, setAdmin] = useState<boolean | null>(null);
  const [mode, setMode] = useState<Mode>("beginner");
  const [query, setQuery] = useState("");
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({});
  const [updateBanner, setUpdateBanner] = useState<{ version: string } | null>(null);
  const updateChecked = useRef(false);

  useEffect(() => { api.isAdmin().then(setAdmin).catch(() => setAdmin(false)); }, []);
  useEffect(() => {
    if (updateChecked.current) return;
    updateChecked.current = true;
    const timer = setTimeout(async () => {
      try { const update = await checkUpdate(); if (update) setUpdateBanner({ version: update.version }); } catch { /* offline or development build */ }
    }, 3000);
    return () => clearTimeout(timer);
  }, []);

  const selected = useMemo(() => nav.flatMap((group) => group.items.map((item) => ({ ...item, group: group.group }))).find((item) => item.id === page), [nav, page]);
  const normalizedQuery = query.trim().toLocaleLowerCase();
  const visibleGroups = nav.map((group) => ({ ...group, items: group.items.filter((item) => !normalizedQuery || item.label.toLocaleLowerCase().includes(normalizedQuery)) })).filter((group) => group.items.length > 0);
  const selectPage = (id: string) => { setPage(id); if (normalizedQuery) setQuery(""); };

  return (
    <div className="layout app-shell">
      <aside className="sidebar" aria-label="Primary navigation">
        <div className="brand-block">
          <div className="brand-mark">AD</div>
          <div><div className="logo">Hyper<span>Optimize</span></div><div className="brand-caption">Windows control center</div></div>
        </div>

        <div className="nav-search-wrap">
          <span aria-hidden="true">⌕</span>
          <input className="nav-search" value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search tools" aria-label="Search tools" />
          {query && <button className="nav-search-clear" onClick={() => setQuery("")} aria-label="Clear search">×</button>}
        </div>

        <nav className="nav-scroll">
          {visibleGroups.map((group) => {
            const isOpen = normalizedQuery || !collapsed[group.group];
            return <section className="nav-group" key={group.group}>
              <button className="nav-group-label" onClick={() => setCollapsed((current) => ({ ...current, [group.group]: !current[group.group] }))} aria-expanded={!!isOpen}>
                <span>{group.group}</span><span className={`group-chevron ${isOpen ? "open" : ""}`}>⌄</span>
              </button>
              {isOpen && group.items.map((item) => <button key={item.id} className={`nav-item ${page === item.id ? "active" : ""}`} onClick={() => selectPage(item.id)} aria-current={page === item.id ? "page" : undefined}>
                <span className="nav-icon" aria-hidden="true">{item.icon}</span><span>{item.label}</span>
              </button>)}
            </section>;
          })}
          {visibleGroups.length === 0 && <div className="nav-empty">No matching tools</div>}
        </nav>

        <div className="sidebar-footer">
          <div className="sidebar-status"><span className={`status-dot ${admin ? "is-ready" : ""}`} />{admin === null ? t("navChecking") : admin ? t("adminBadge") : t("userBadge")}</div>
          <div className="mode-toggle">
            <button className={mode === "beginner" ? "on" : ""} onClick={() => setMode("beginner")}>{t("beginner")}</button>
            <button className={mode === "expert" ? "on" : ""} onClick={() => setMode("expert")}>{t("expert")}</button>
          </div>
          <div className="sidebar-bottom-row">
            <select className="lang-select" value={lang} onChange={(event) => setLang(event.target.value as Lang)} aria-label="Language">
              {Object.entries(LANG_NAMES).map(([code, name]) => <option key={code} value={code}>{name}</option>)}
            </select>
            <button className="social-btn" onClick={() => api.openPath("https://discord.gg/vFaKsVuxKP")} title="Discord">D</button>
            <button className="social-btn x" onClick={() => api.openPath("https://x.com/zCrxticxl")} title="X">X</button>
          </div>
        </div>
      </aside>

      <main className="main">
        <header className="app-topbar">
          <div><div className="app-crumb">AD HYPEROPTIMIZE <span>/</span> {selected?.group}</div><div className="app-page-name">{selected?.label}</div></div>
          <div className="topbar-actions"><span className="safe-pill"><span className="status-dot is-ready" />Revertible changes</span><span className={`mode-pill ${mode}`}>{mode === "beginner" ? t("beginner") : t("expert")}</span></div>
        </header>
        {updateBanner && <div className="update-banner"><div><b>↻ {t("navUpdateAvailable")}</b><span>v{updateBanner.version} {t("navUpdateReady")}</span></div><button className="btn small" onClick={() => { setPage("updates"); setUpdateBanner(null); }}>{t("navInstallUpdate")}</button><button className="icon-button" onClick={() => setUpdateBanner(null)} aria-label="Dismiss update">×</button></div>}
        <div className="page-content">
          {page === "dashboard" && <Dashboard mode={mode} go={setPage} />}
          {page === "hardware" && <Hardware mode={mode} />}{page === "monitor" && <Monitor />}{page === "latency" && <Latency />}
          {page === "processes" && <Processes />}{page === "startup" && <Startup admin={!!admin} />}{page === "schedtasks" && <ScheduledTasks admin={!!admin} />}
          {page === "optimize" && <Optimize mode={mode} admin={!!admin} />}{page === "profiles" && <Profiles />}{page === "gameprofiles" && <GameProfiles />}
          {page === "cleanup" && <Cleanup />}{page === "regclean" && <RegClean admin={!!admin} />}{page === "gputweaks" && <GpuTweaks admin={!!admin} />}
          {page === "nvcontrol" && <NvidiaControlPanel />}{page === "diskanalyzer" && <DiskAnalyzer />}{page === "bootopt" && <BootOptimizer admin={!!admin} />}
          {page === "privacy" && <PrivacyCenter admin={!!admin} />}{page === "services" && <ServicesManager admin={!!admin} />}{page === "healthcheck" && <HealthCheck admin={!!admin} />}
          {page === "updates" && <Updates admin={!!admin} />}{page === "perftweaks" && <PerfTweaks admin={!!admin} />}{page === "hwmonitor" && <HwMonitor />}
          {page === "powerplan" && <PowerPlan admin={!!admin} />}{page === "uninstaller" && <AppUninstaller admin={!!admin} />}{page === "ctxmenu" && <CtxMenuCleaner admin={!!admin} />}
          {page === "debloater" && <Debloater admin={!!admin} />}{page === "drivers" && <DriverManager />}{page === "gameboost" && <GameBooster admin={!!admin} />}
          {page === "security" && <Security mode={mode} />}{page === "benchmark" && <Benchmark />}{page === "reports" && <Reports />}
          {page === "autoopt" && <AutoOptimizer admin={!!admin} />}{page === "restorepoints" && <RestorePointManager admin={!!admin} />}
          {page === "softinstaller" && <SoftwareInstaller />}{page === "pcconfig" && <PcConfigurator />}
        </div>
      </main>
    </div>
  );
}

export default function App() { return <LangProvider><AppInner /></LangProvider>; }
