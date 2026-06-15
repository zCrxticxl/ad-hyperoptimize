import React, { useEffect, useRef, useState } from "react";
import { check as checkUpdate } from "@tauri-apps/plugin-updater";
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
import SoftwareInstaller from "./pages/SoftwareInstaller";

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
      { id: "debloater",    icon: "🚀", label: "Quick Setup" },
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
      { id: "softinstaller",  icon: "📦", label: "Software Installer" },
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
  const [updateBanner, setUpdateBanner] = useState<{ version: string } | null>(null);
  const updateChecked = useRef(false);

  useEffect(() => {
    api.isAdmin().then(setAdmin).catch(() => setAdmin(false));
  }, []);

  // Auto-check for updates on startup (only fires once, only in production builds)
  useEffect(() => {
    if (updateChecked.current) return;
    updateChecked.current = true;
    const timer = setTimeout(async () => {
      try {
        const update = await checkUpdate();
        if (update) setUpdateBanner({ version: update.version });
      } catch {
        // silently ignore — dev mode, offline, etc.
      }
    }, 3000); // 3s delay so the app feels snappy on launch
    return () => clearTimeout(timer);
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

          <div style={{ display: "flex", gap: 8, marginTop: 10, justifyContent: "center" }}>
            {/* Discord */}
            <button
              onClick={() => api.openPath("https://discord.gg/vFaKsVuxKP")}
              title="Discord Server"
              style={{ background: "#5865F2", border: "none", borderRadius: 7, padding: "5px 8px", cursor: "pointer", display: "flex", alignItems: "center", justifyContent: "center" }}
            >
              <svg width="16" height="16" viewBox="0 0 24 24" fill="white" xmlns="http://www.w3.org/2000/svg">
                <path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057c.002.022.015.045.034.06a19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/>
              </svg>
            </button>
            {/* X / Twitter */}
            <button
              onClick={() => api.openPath("https://x.com/zCrxticxl")}
              title="@zCrxticxl on X"
              style={{ background: "#000", border: "1px solid #333", borderRadius: 7, padding: "5px 8px", cursor: "pointer", display: "flex", alignItems: "center", justifyContent: "center" }}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="white" xmlns="http://www.w3.org/2000/svg">
                <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-4.714-6.231-5.401 6.231H2.744l7.73-8.835L1.254 2.25H8.08l4.713 6.231 5.45-6.231Zm-1.161 17.52h1.833L7.084 4.126H5.117L17.083 19.77Z"/>
              </svg>
            </button>
          </div>
        </div>
      </aside>

      <main className="main">
        {/* Update available banner */}
        {updateBanner && (
          <div style={{
            display: "flex", alignItems: "center", gap: 12,
            padding: "10px 16px", marginBottom: 14,
            background: "rgba(0,140,255,0.10)", border: "1px solid rgba(0,140,255,0.35)",
            borderRadius: 8, fontSize: 13,
          }}>
            <span style={{ fontWeight: 700, color: "var(--accent)" }}>⟳ Update available</span>
            <span className="muted">v{updateBanner.version} is ready to install</span>
            <button
              className="btn small"
              style={{ marginLeft: "auto" }}
              onClick={() => { setPage("updates"); setUpdateBanner(null); }}
            >
              Install update →
            </button>
            <button
              className="btn small ghost"
              style={{ padding: "3px 8px" }}
              onClick={() => setUpdateBanner(null)}
            >✕</button>
          </div>
        )}
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
        {page === "softinstaller"  && <SoftwareInstaller />}
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
