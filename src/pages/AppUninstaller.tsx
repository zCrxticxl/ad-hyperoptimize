import React, { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";

function fmtMb(mb: number) {
  if (!mb) return "";
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${mb} MB`;
}

export default function AppUninstaller({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [apps, setApps]             = useState<any[]>([]);
  const [loading, setLoading]       = useState(true);
  const [search, setSearch]         = useState("");
  const [busy, setBusy]             = useState<string | null>(null);
  const [log, setLog]               = useState<Record<string, string>>({});
  // leftover scanning
  const [scanning, setScanning]     = useState<string | null>(null);
  const [leftovers, setLeftovers]   = useState<Record<string, any[]>>({});
  const [selLeft, setSelLeft]       = useState<Record<string, Set<string>>>({});
  const [cleaning, setCleaning]     = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    try {
      const d = await api.uninstallerList();
      setApps(d.apps ?? []);
    } finally { setLoading(false); }
  };

  useEffect(() => { load(); }, []);

  const filtered = useMemo(() => {
    if (!search.trim()) return apps;
    const q = search.toLowerCase();
    return apps.filter(a =>
      a.name?.toLowerCase().includes(q) ||
      a.publisher?.toLowerCase().includes(q)
    );
  }, [apps, search]);

  const uninstall = async (app: any) => {
    if (!app.uninstallString) return;
    if (!window.confirm(`${t("uninstConfirmTitle")} "${app.name}"?\n\n${t("uninstConfirmBody")}`)) return;
    setBusy(app.name);
    try {
      const msg = await api.uninstallApp(app.uninstallString);
      setLog(prev => ({ ...prev, [app.name]: msg }));
    } catch (e: any) {
      setLog(prev => ({ ...prev, [app.name]: String(e) }));
    } finally { setBusy(null); }
  };

  const scanLeft = async (app: any) => {
    setScanning(app.name);
    try {
      const d = await api.scanLeftovers(app.name, app.publisher ?? "", app.installLocation ?? "");
      const items: any[] = d.leftovers ?? [];
      setLeftovers(prev => ({ ...prev, [app.name]: items }));
      setSelLeft(prev => ({ ...prev, [app.name]: new Set(items.map((i: any) => i.path)) }));
    } finally { setScanning(null); }
  };

  const cleanLeft = async (appName: string) => {
    const sel = selLeft[appName];
    if (!sel?.size) return;
    setCleaning(appName);
    try {
      const msg = await api.cleanLeftovers([...sel]);
      setLog(prev => ({ ...prev, [appName]: msg }));
      setLeftovers(prev => ({ ...prev, [appName]: [] }));
    } catch (e: any) {
      setLog(prev => ({ ...prev, [appName]: String(e) }));
    } finally { setCleaning(null); }
  };

  const toggleLeft = (appName: string, path: string) => {
    setSelLeft(prev => {
      const next = new Set(prev[appName] ?? []);
      if (next.has(path)) next.delete(path); else next.add(path);
      return { ...prev, [appName]: next };
    });
  };

  return (
    <>
      <div className="page-title">🗑 {t("uninstTitle")}</div>
      <div className="page-sub">
        {t("uninstSub")}
        · {apps.length} {t("uninstAppsInstalled")}
      </div>

      <Card title="">
        <div className="row" style={{ gap: 8, marginBottom: 12 }}>
          <input
            placeholder={`${t("uninstSearchPlaceholder")} ${apps.length} ${t("uninstAppsWord")}…`}
            value={search}
            onChange={e => setSearch(e.target.value)}
            style={{ flex: 1, padding: "5px 10px", fontSize: 13, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}
          />
          <button className="btn small ghost" onClick={load} disabled={loading}>↺ {t("uninstRefresh")}</button>
        </div>

        {loading ? (
          <div className="row" style={{ gap: 10 }}><Spinner /><span className="muted">{t("uninstScanning")}</span></div>
        ) : filtered.length === 0 ? (
          <div className="muted">{t("uninstNoApps")}{search ? ` ${t("uninstMatchingFilter")}` : ""}.</div>
        ) : (
          <div style={{ maxHeight: "65vh", overflowY: "auto" }}>
            {filtered.map((app: any) => {
              const appLog = log[app.name];
              const appLeft = leftovers[app.name] ?? [];
              const appSel = selLeft[app.name] ?? new Set<string>();
              const wasLaunched = !!appLog;

              return (
                <div key={app.name} style={{ borderBottom: "1px solid var(--border)", padding: "8px 0" }}>
                  <div className="row" style={{ alignItems: "center", gap: 10 }}>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontWeight: 600, fontSize: 13 }}>{app.name}</div>
                      <div className="muted" style={{ fontSize: 11 }}>
                        {app.publisher && <span>{app.publisher}</span>}
                        {app.version && <span> · v{app.version}</span>}
                        {app.sizeMb > 0 && <span> · {fmtMb(app.sizeMb)}</span>}
                      </div>
                    </div>

                    {wasLaunched ? (
                      <button
                        className="btn small ghost"
                        onClick={() => scanLeft(app)}
                        disabled={scanning === app.name}
                      >
                        {scanning === app.name ? <><Spinner /> {t("uninstScanningShort")}</> : `🔍 ${t("uninstScanLeftovers")}`}
                      </button>
                    ) : (
                      <button
                        className="btn small ghost danger"
                        onClick={() => uninstall(app)}
                        disabled={busy === app.name || !app.uninstallString}
                      >
                        {busy === app.name ? "…" : t("uninstUninstallBtn")}
                      </button>
                    )}
                  </div>

                  {appLog && (
                    <div className="mono muted" style={{ fontSize: 11, marginTop: 4 }}>{appLog}</div>
                  )}

                  {appLeft.length > 0 && (
                    <div style={{ marginTop: 8, padding: 8, background: "var(--bg2)", borderRadius: 4, border: "1px solid var(--border)" }}>
                      <div className="row" style={{ marginBottom: 6, alignItems: "center", gap: 8 }}>
                        <span style={{ fontSize: 12, fontWeight: 600 }}>{appLeft.length} {t("uninstLeftoversFound")}</span>
                        <div style={{ flex: 1 }} />
                        <button
                          className="btn small danger"
                          onClick={() => cleanLeft(app.name)}
                          disabled={!appSel.size || cleaning === app.name}
                        >
                          {cleaning === app.name ? "…" : `🗑 ${t("uninstClean")} (${appSel.size})`}
                        </button>
                      </div>
                      {appLeft.map((item: any) => (
                        <label key={item.path} className="row" style={{ gap: 8, padding: "2px 0", cursor: "pointer", alignItems: "center" }}>
                          <input
                            type="checkbox"
                            checked={appSel.has(item.path)}
                            onChange={() => toggleLeft(app.name, item.path)}
                          />
                          <span style={{ fontSize: 11, color: item.type === "registry" ? "var(--orange)" : "var(--fg)" }}>
                            [{item.type}]
                          </span>
                          <span className="mono muted" style={{ fontSize: 10, flex: 1, wordBreak: "break-all" }}>
                            {item.path}
                          </span>
                        </label>
                      ))}
                    </div>
                  )}

                  {wasLaunched && appLeft.length === 0 && scanning !== app.name && (
                    <div className="muted" style={{ fontSize: 11, marginTop: 4 }}>
                      {t("uninstNoLeftovers")}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </Card>
    </>
  );
}
