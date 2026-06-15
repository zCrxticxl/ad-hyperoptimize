import React, { useEffect, useState, useMemo } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";

type App = {
  id: string;
  name: string;
  category: string;
  desc: string;
  wingetId: string;
  icon: string;
  recommended: boolean;
};

type InstallStatus = "idle" | "installing" | "done" | "error";

type AppState = {
  installed: boolean;
  status: InstallStatus;
  message: string;
};

const CATEGORIES: { key: string; label: string; icon: string }[] = [
  { key: "all",       label: "All",           icon: "⬡" },
  { key: "gaming",    label: "Gaming",         icon: "🎮" },
  { key: "comms",     label: "Communication",  icon: "💬" },
  { key: "browsers",  label: "Browsers",       icon: "🌐" },
  { key: "utilities", label: "Utilities",      icon: "🔧" },
  { key: "tools",     label: "Tools",          icon: "📊" },
];

const STATUS_COLOR: Record<InstallStatus, string> = {
  idle:       "transparent",
  installing: "var(--accent)",
  done:       "var(--green)",
  error:      "var(--red)",
};

const STATUS_ICON: Record<InstallStatus, string> = {
  idle:       "",
  installing: "⟳",
  done:       "✓",
  error:      "✕",
};

export default function SoftwareInstaller() {
  const [apps, setApps]       = useState<App[]>([]);
  const [states, setStates]   = useState<Record<string, AppState>>({});
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [cat, setCat]         = useState("all");
  const [loading, setLoading] = useState(true);
  const [installing, setInstalling] = useState(false);
  const [log, setLog]         = useState("");

  // load catalog + check installed
  useEffect(() => {
    (async () => {
      const [catalog, installed] = await Promise.all([
        api.swCatalog(),
        api.swCheckInstalled(),
      ]);
      setApps(catalog);
      const init: Record<string, AppState> = {};
      for (const a of catalog) {
        init[a.wingetId] = {
          installed: !!installed[a.wingetId],
          status: "idle",
          message: "",
        };
      }
      setStates(init);
      setLoading(false);
    })();
  }, []);

  // listen for install progress events
  useEffect(() => {
    const unlisten = listen<any>("sw-install-progress", e => {
      const { wingetId, status, message } = e.payload;
      if (wingetId === "__done__") {
        setInstalling(false);
        setLog(message);
        return;
      }
      setStates(prev => ({
        ...prev,
        [wingetId]: {
          ...prev[wingetId],
          status: status as InstallStatus,
          message,
          installed: status === "done" ? true : prev[wingetId]?.installed,
        },
      }));
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  const filtered = useMemo(() =>
    cat === "all" ? apps : apps.filter(a => a.category === cat),
    [apps, cat]
  );

  const toggle = (wingetId: string) => {
    setSelected(prev => {
      const n = new Set(prev);
      n.has(wingetId) ? n.delete(wingetId) : n.add(wingetId);
      return n;
    });
  };

  const selectRecommended = () => {
    setSelected(new Set(apps.filter(a => a.recommended).map(a => a.wingetId)));
  };

  const selectCategory = () => {
    setSelected(new Set(filtered.map(a => a.wingetId)));
  };

  const clearAll = () => setSelected(new Set());

  const install = async () => {
    if (!selected.size || installing) return;
    setInstalling(true);
    setLog("");
    // reset status for selected
    setStates(prev => {
      const n = { ...prev };
      selected.forEach(id => { n[id] = { ...n[id], status: "idle", message: "" }; });
      return n;
    });
    await api.swInstall([...selected]);
  };

  const selectedNotInstalled = [...selected].filter(id => !states[id]?.installed);

  return (
    <>
      <div className="page-title">📦 Software Installer</div>
      <div className="page-sub">
        Install curated apps via winget — no browser, no installer hunting.
        Select what you need and install everything in one click.
      </div>

      {/* Category tabs */}
      <div style={{ display: "flex", gap: 6, marginBottom: 14, flexWrap: "wrap" }}>
        {CATEGORIES.map(c => (
          <button
            key={c.key}
            className={`btn small ${cat === c.key ? "" : "ghost"}`}
            onClick={() => setCat(c.key)}
          >
            {c.icon} {c.label}
          </button>
        ))}
      </div>

      {/* Toolbar */}
      <div style={{ display: "flex", gap: 8, marginBottom: 12, alignItems: "center" }}>
        <button className="btn small ghost" onClick={selectRecommended}>⭐ Recommended</button>
        <button className="btn small ghost" onClick={selectCategory}>
          Select {cat === "all" ? "All" : CATEGORIES.find(c => c.key === cat)?.label}
        </button>
        {selected.size > 0 && (
          <button className="btn small ghost" onClick={clearAll}>Clear</button>
        )}
        <div style={{ flex: 1 }} />
        <button
          className="btn"
          disabled={!selected.size || installing}
          onClick={install}
          style={{ minWidth: 160 }}
        >
          {installing
            ? <><Spinner /> Installing…</>
            : `⬇ Install (${selected.size})`}
        </button>
      </div>

      {/* App grid */}
      {loading ? (
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <Spinner /> <span className="muted">Checking installed apps…</span>
        </div>
      ) : (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(320px, 1fr))", gap: 8 }}>
          {filtered.map(app => {
            const st = states[app.wingetId] ?? { installed: false, status: "idle", message: "" };
            const isSelected = selected.has(app.wingetId);
            const isActive = st.status !== "idle";
            const statusColor = STATUS_COLOR[st.status];

            return (
              <div
                key={app.id}
                onClick={() => !installing && toggle(app.wingetId)}
                style={{
                  display:      "flex",
                  alignItems:   "center",
                  gap:          12,
                  padding:      "10px 12px",
                  borderRadius: 8,
                  border:       `1px solid ${isSelected ? "var(--accent)" : "var(--border)"}`,
                  background:   isSelected ? "rgba(0,140,255,0.07)" : "var(--bg2)",
                  cursor:       installing ? "default" : "pointer",
                  transition:   "border-color 0.15s, background 0.15s",
                  position:     "relative",
                  overflow:     "hidden",
                }}
              >
                {/* left accent bar for status */}
                {isActive && (
                  <div style={{
                    position: "absolute", left: 0, top: 0, bottom: 0,
                    width: 3, background: statusColor,
                  }} />
                )}

                {/* checkbox */}
                <input
                  type="checkbox"
                  checked={isSelected}
                  onChange={() => {}}
                  onClick={e => e.stopPropagation()}
                  style={{ pointerEvents: "none", flexShrink: 0 }}
                />

                {/* icon */}
                <span style={{ fontSize: 22, flexShrink: 0 }}>{app.icon}</span>

                {/* info */}
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                    <span style={{ fontWeight: 600, fontSize: 13 }}>{app.name}</span>
                    {app.recommended && (
                      <span style={{ fontSize: 10, color: "var(--yellow)", fontWeight: 700 }}>★</span>
                    )}
                    {st.installed && st.status === "idle" && (
                      <span style={{ fontSize: 10, color: "var(--green)", fontWeight: 700 }}>✓ installed</span>
                    )}
                  </div>
                  <div className="muted" style={{ fontSize: 11, marginTop: 1, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                    {isActive && st.message
                      ? <span style={{ color: statusColor }}>{STATUS_ICON[st.status]} {st.message}</span>
                      : app.desc}
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {log && (
        <div className="muted" style={{ fontSize: 12, marginTop: 12 }}>{log}</div>
      )}

      <div className="muted" style={{ fontSize: 11, marginTop: 16 }}>
        ★ = Recommended for a fresh gaming setup &nbsp;·&nbsp;
        Already installed apps can be reinstalled to repair/upgrade &nbsp;·&nbsp;
        Requires internet connection
      </div>
    </>
  );
}
