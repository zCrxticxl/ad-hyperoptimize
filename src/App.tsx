import React, { useEffect, useState } from "react";
import { api } from "./api";
import { LangProvider, useLang } from "./i18n";
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

type NavItem = { id: string; icon: string; label: string };
type NavGroup = { group: string; items: NavItem[] };

const NAV: NavGroup[] = [
  {
    group: "Overview",
    items: [
      { id: "dashboard",   icon: "⌂",  label: "Dashboard" },
      { id: "hardware",    icon: "▦",  label: "Hardware" },
      { id: "monitor",     icon: "∿",  label: "Live Monitor" },
      { id: "hwmonitor",   icon: "🌡", label: "HW Monitor" },
      { id: "healthcheck", icon: "✚",  label: "Health Check" },
    ],
  },
  {
    group: "Performance",
    items: [
      { id: "autoopt",    icon: "✨", label: "Auto-Optimize" },
      { id: "perftweaks", icon: "⚡", label: "Perf Tweaks" },
      { id: "optimize",   icon: "🚀", label: "Optimize" },
      { id: "gameboost",  icon: "🎮", label: "Game Booster" },
      { id: "powerplan",  icon: "🔋", label: "Power Plan" },
      { id: "latency",    icon: "⚠",  label: "Latency" },
      { id: "gputweaks",    icon: "▣",  label: "GPU Tweaks" },
      { id: "profiles",     icon: "◎",  label: "Profiles" },
      { id: "gameprofiles", icon: "🎮", label: "Game Profiles" },
    ],
  },
  {
    group: "Cleanup",
    items: [
      { id: "cleanup",      icon: "🧹", label: "Cleanup" },
      { id: "uninstaller",  icon: "🗑", label: "Uninstaller" },
      { id: "diskanalyzer", icon: "◉",  label: "Disk Analyzer" },
      { id: "regclean",     icon: "⎔",  label: "Reg Cleaner" },
      { id: "debloater",    icon: "🧼", label: "Debloater" },
      { id: "ctxmenu",      icon: "☰",  label: "Context Menu" },
    ],
  },
  {
    group: "Privacy & Security",
    items: [
      { id: "privacy",  icon: "🔒", label: "Privacy" },
      { id: "security", icon: "🛡", label: "Security" },
    ],
  },
  {
    group: "System",
    items: [
      { id: "processes",     icon: "≣",  label: "Processes" },
      { id: "startup",       icon: "▶",  label: "Startup" },
      { id: "services",      icon: "⚙",  label: "Services" },
      { id: "schedtasks",    icon: "⏲",  label: "Sched. Tasks" },
      { id: "drivers",       icon: "🔧", label: "Drivers" },
      { id: "bootopt",       icon: "⏻",  label: "Boot Optimizer" },
      { id: "restorepoints", icon: "🛟", label: "Restore Points" },
      { id: "updates",       icon: "⟳",  label: "Updates" },
    ],
  },
  {
    group: "Reports",
    items: [
      { id: "benchmark", icon: "⏱",  label: "Benchmark" },
      { id: "reports",   icon: "📄", label: "Reports" },
    ],
  },
];

export type Mode = "beginner" | "expert";

function AppInner() {
  const { lang, setLang, t } = useLang();
  const [page, setPage] = useState<string>("dashboard");
  const [admin, setAdmin] = useState<boolean | null>(null);
  const [mode, setMode] = useState<Mode>("beginner");

  useEffect(() => {
    api.isAdmin().then(setAdmin).catch(() => setAdmin(false));
  }, []);

  return (
    <div className="layout">
      <aside className="sidebar">
        <div className="logo">AD <span>Hyper</span>Optimize</div>

        <div style={{ overflowY: "auto", flex: 1 }}>
          {NAV.map((group) => (
            <div key={group.group}>
              <div style={{
                fontSize: 10,
                fontWeight: 700,
                letterSpacing: "0.08em",
                textTransform: "uppercase",
                color: "var(--muted)",
                padding: "14px 14px 4px",
                opacity: 0.6,
              }}>
                {group.group}
              </div>
              {group.items.map((p) => (
                <div
                  key={p.id}
                  className={`nav-item ${page === p.id ? "active" : ""}`}
                  onClick={() => setPage(p.id)}
                >
                  <span className="nav-icon">{p.icon}</span> {p.label}
                </div>
              ))}
            </div>
          ))}
        </div>

        <div className="sidebar-footer">
          <div className="mode-toggle" style={{ marginBottom: 8 }}>
            <button className={lang === "de" ? "on" : ""} onClick={() => setLang("de")}>DE</button>
            <button className={lang === "en" ? "on" : ""} onClick={() => setLang("en")}>EN</button>
          </div>

          <div className="mode-toggle" style={{ marginBottom: 10 }}>
            <button className={mode === "beginner" ? "on" : ""} onClick={() => setMode("beginner")}>
              {t("beginner")}
            </button>
            <button className={mode === "expert" ? "on" : ""} onClick={() => setMode("expert")}>
              {t("expert")}
            </button>
          </div>

          {admin === null ? "checking…" : admin ? (
            <span className="admin-badge admin-yes">{t("adminBadge")}</span>
          ) : (
            <span className="admin-badge admin-no">{t("userBadge")}</span>
          )}
          {!admin && admin !== null && (
            <div style={{ marginTop: 6, fontSize: 11 }}>{t("adminHintSidebar")}</div>
          )}
        </div>
      </aside>

      <main className="main">
        {page === "dashboard"      && <Dashboard mode={mode} go={setPage} />}
        {page === "hardware"       && <Hardware mode={mode} />}
        {page === "monitor"        && <Monitor />}
        {page === "latency"        && <Latency />}
        {page === "processes"      && <Processes />}
        {page === "startup"        && <Startup admin={!!admin} />}
        {page === "schedtasks"     && <ScheduledTasks admin={!!admin} />}
        {page === "optimize"       && <Optimize mode={mode} admin={!!admin} />}
        {page === "profiles"       && <Profiles />}
        {page === "gameprofiles"   && <GameProfiles />}
        {page === "cleanup"        && <Cleanup />}
        {page === "regclean"       && <RegClean admin={!!admin} />}
        {page === "gputweaks"      && <GpuTweaks admin={!!admin} />}
        {page === "diskanalyzer"   && <DiskAnalyzer />}
        {page === "bootopt"        && <BootOptimizer admin={!!admin} />}
        {page === "privacy"        && <PrivacyCenter admin={!!admin} />}
        {page === "services"       && <ServicesManager admin={!!admin} />}
        {page === "healthcheck"    && <HealthCheck admin={!!admin} />}
        {page === "updates"        && <Updates admin={!!admin} />}
        {page === "perftweaks"     && <PerfTweaks admin={!!admin} />}
        {page === "hwmonitor"      && <HwMonitor />}
        {page === "powerplan"      && <PowerPlan admin={!!admin} />}
        {page === "uninstaller"    && <AppUninstaller admin={!!admin} />}
        {page === "ctxmenu"        && <CtxMenuCleaner admin={!!admin} />}
        {page === "debloater"      && <Debloater admin={!!admin} />}
        {page === "drivers"        && <DriverManager />}
        {page === "gameboost"      && <GameBooster admin={!!admin} />}
        {page === "security"       && <Security mode={mode} />}
        {page === "benchmark"      && <Benchmark />}
        {page === "reports"        && <Reports />}
        {page === "autoopt"        && <AutoOptimizer admin={!!admin} />}
        {page === "restorepoints"  && <RestorePointManager admin={!!admin} />}
      </main>
    </div>
  );
}

export default function App() {
  return (
    <LangProvider>
      <AppInner />
    </LangProvider>
  );
}
