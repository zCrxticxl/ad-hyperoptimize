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

const PAGES = [
  { id: "dashboard",    icon: "⌂",  label: "Dashboard" },
  { id: "hardware",     icon: "▦",  label: "Hardware" },
  { id: "monitor",      icon: "∿",  label: "Live Monitor" },
  { id: "latency",      icon: "⚠",  label: "Latency" },
  { id: "perftweaks",  icon: "⚡", label: "Perf Tweaks" },
  { id: "hwmonitor",   icon: "🌡", label: "HW Monitor" },
  { id: "processes",    icon: "≣",  label: "Processes" },
  { id: "startup",      icon: "🚀", label: "Startup" },
  { id: "schedtasks",   icon: "⏲",  label: "Sched. Tasks" },
  { id: "optimize",     icon: "⚡", label: "Optimize" },
  { id: "profiles",     icon: "◎",  label: "Profiles" },
  { id: "cleanup",      icon: "🧹", label: "Cleanup" },
  { id: "regclean",     icon: "⎔",  label: "Reg Cleaner" },
  { id: "gputweaks",    icon: "▣",  label: "GPU Tweaks" },
  { id: "diskanalyzer", icon: "◉",  label: "Disk Analyzer" },
  { id: "bootopt",      icon: "⏻",  label: "Boot Optimizer" },
  { id: "privacy",      icon: "🔒", label: "Privacy" },
  { id: "services",     icon: "⚙",  label: "Services" },
  { id: "healthcheck",  icon: "✚",  label: "Health Check" },
  { id: "updates",      icon: "⟳",  label: "Updates" },
  { id: "security",     icon: "🛡", label: "Security" },
  { id: "benchmark",    icon: "⏱",  label: "Benchmark" },
  { id: "reports",      icon: "📄", label: "Reports" },
] as const;

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
          {PAGES.map((p) => (
            <div
              key={p.id}
              className={`nav-item ${page === p.id ? "active" : ""}`}
              onClick={() => setPage(p.id)}
            >
              <span className="nav-icon">{p.icon}</span> {p.label}
            </div>
          ))}
        </div>

        <div className="sidebar-footer">
          {/* Language toggle */}
          <div className="mode-toggle" style={{ marginBottom: 8 }}>
            <button className={lang === "de" ? "on" : ""} onClick={() => setLang("de")}>DE</button>
            <button className={lang === "en" ? "on" : ""} onClick={() => setLang("en")}>EN</button>
          </div>

          {/* Beginner / Expert */}
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
        {page === "dashboard"    && <Dashboard mode={mode} go={setPage} />}
        {page === "hardware"     && <Hardware mode={mode} />}
        {page === "monitor"      && <Monitor />}
        {page === "latency"      && <Latency />}
        {page === "processes"    && <Processes />}
        {page === "startup"      && <Startup admin={!!admin} />}
        {page === "schedtasks"   && <ScheduledTasks admin={!!admin} />}
        {page === "optimize"     && <Optimize mode={mode} admin={!!admin} />}
        {page === "profiles"     && <Profiles />}
        {page === "cleanup"      && <Cleanup />}
        {page === "regclean"     && <RegClean admin={!!admin} />}
        {page === "gputweaks"    && <GpuTweaks admin={!!admin} />}
        {page === "diskanalyzer" && <DiskAnalyzer />}
        {page === "bootopt"      && <BootOptimizer admin={!!admin} />}
        {page === "privacy"      && <PrivacyCenter admin={!!admin} />}
        {page === "services"     && <ServicesManager admin={!!admin} />}
        {page === "healthcheck"  && <HealthCheck admin={!!admin} />}
        {page === "updates"      && <Updates admin={!!admin} />}
        {page === "perftweaks"   && <PerfTweaks admin={!!admin} />}
        {page === "hwmonitor"    && <HwMonitor />}
        {page === "security"     && <Security mode={mode} />}
        {page === "benchmark"    && <Benchmark />}
        {page === "reports"      && <Reports />}
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
