import React, { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang, TKey } from "../i18n";
import { APP_CATEGORIES, AppCategory, AppSort, categorize, sortApps } from "../lib/appCategory";

/** Stable row identity: the registry key path when the backend provides it,
 * otherwise hive + name + version + uninstall command (names alone collide). */
const appKey = (a: any): string =>
  a.keyPath ?? a.regKey ?? a.psPath ?? `${a.hive ?? ""}|${a.name ?? ""}|${a.version ?? ""}|${a.uninstallString ?? ""}`;

type UninstStatus = { state: "launched" | "error"; msg: string; app: any };

function fmtMb(mb: number) {
  if (!mb) return "";
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
  return `${mb} MB`;
}

export default function AppUninstaller({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [apps, setApps]             = useState<any[]>([]);
  const [loading, setLoading]       = useState(true);
  const [loadErr, setLoadErr]       = useState("");
  const [search, setSearch]         = useState("");
  const [category, setCategory]     = useState<AppCategory | null>(null);
  const [sortBy, setSortBy]         = useState<AppSort>("name");
  const [busy, setBusy]             = useState<string | null>(null);
  const [log, setLog]               = useState<Record<string, string>>({});
  // Per-app uninstall outcome, keyed by appKey: "error" must never unlock the
  // leftover scan that only makes sense after an uninstaller actually ran.
  const [status, setStatus]         = useState<Record<string, UninstStatus>>({});
  const statusRef = useRef<Record<string, UninstStatus>>({});
  const putStatus = (key: string, s: UninstStatus) => {
    statusRef.current = { ...statusRef.current, [key]: s };
    setStatus(statusRef.current);
  };
  // leftover scanning
  const [scanning, setScanning]     = useState<string | null>(null);
  const [leftovers, setLeftovers]   = useState<Record<string, any[]>>({});
  const [selLeft, setSelLeft]       = useState<Record<string, Set<string>>>({});
  const [cleaning, setCleaning]     = useState<string | null>(null);
  const [confirmClean, setConfirmClean] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setLoadErr("");
    try {
      const d = await api.uninstallerList();
      const fresh: any[] = d.apps ?? [];
      // Keep rows of apps uninstalled in this session so their leftovers can
      // still be scanned after they vanish from the registry list.
      const present = new Set(fresh.map(appKey));
      const gone = Object.entries(statusRef.current)
        .filter(([k, s]) => s.state === "launched" && !present.has(k))
        .map(([, s]) => ({ ...s.app, removed: true }));
      setApps([...fresh, ...gone]);
    } catch (e: any) {
      setLoadErr(String(e));
      setApps([]);
    } finally { setLoading(false); }
  };

  useEffect(() => { load(); }, []);

  const withCat = useMemo(() => apps.map(a => ({ ...a, category: categorize(a) })), [apps]);
  const counts = useMemo(() => {
    const c: Partial<Record<AppCategory, number>> = {};
    for (const a of withCat) c[a.category as AppCategory] = (c[a.category as AppCategory] ?? 0) + 1;
    return c;
  }, [withCat]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    const hits = withCat.filter(a =>
      (!category || a.category === category) &&
      (!q || a.name?.toLowerCase().includes(q) || a.publisher?.toLowerCase().includes(q))
    );
    return sortApps(hits, sortBy);
  }, [withCat, search, category, sortBy]);

  const uninstall = async (app: any) => {
    if (!app.uninstallString) return;
    if (!window.confirm(`${t("uninstConfirmTitle")} "${app.name}"?\n\n${t("uninstConfirmBody")}`)) return;
    const key = appKey(app);
    setBusy(key);
    let ok = false;
    try {
      const msg = await api.uninstallApp(app.uninstallString, app.hive === "HKCU");
      putStatus(key, { state: "launched", msg, app });
      ok = true;
    } catch (e: any) {
      putStatus(key, { state: "error", msg: String(e), app });
    } finally { setBusy(null); }
    if (ok) await load();
  };

  const scanLeft = async (app: any) => {
    setScanning(app.name);
    try {
      const d = await api.scanLeftovers(app.name, app.publisher ?? "", app.installLocation ?? "");
      const items: any[] = d.leftovers ?? [];
      setLeftovers(prev => ({ ...prev, [app.name]: items }));
      setSelLeft(prev => ({ ...prev, [app.name]: new Set<string>() }));
    } catch (error) {
      setLog(prev => ({ ...prev, [app.name]: String(error) }));
    } finally { setScanning(null); }
  };

  const cleanLeft = async (appName: string) => {
    const sel = selLeft[appName];
    if (!sel?.size) return;
    setCleaning(appName);
    setConfirmClean(null);
    try {
      const msg = await api.cleanLeftovers([...sel]);
      setLog(prev => ({ ...prev, [appName]: msg }));
      setLeftovers(prev => ({ ...prev, [appName]: (prev[appName] ?? []).filter(item => !sel.has(item.path)) }));
      setSelLeft(prev => ({ ...prev, [appName]: new Set<string>() }));
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
      <h1 className="page-title">🗑 {t("uninstTitle")}</h1>
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
          <select value={sortBy} onChange={e => setSortBy(e.target.value as AppSort)} aria-label={t("uninstSort")}
            style={{ padding: "5px 8px", fontSize: 12, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}>
            {(["name", "size", "date", "publisher"] as AppSort[]).map(k => (
              <option key={k} value={k}>{t(`uninstSort_${k}` as TKey)}</option>
            ))}
          </select>
          <button className="btn small ghost" onClick={load} disabled={loading}>↺ {t("uninstRefresh")}</button>
        </div>
        {!loading && apps.length > 0 && (
          <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 12 }}>
            <button className={`btn small ${category ? "ghost" : ""}`} onClick={() => setCategory(null)}>
              {t("uninstCatAll")} ({apps.length})
            </button>
            {APP_CATEGORIES.filter(c => counts[c]).map(c => (
              <button key={c} className={`btn small ${category === c ? "" : "ghost"}`} onClick={() => setCategory(category === c ? null : c)}>
                {t(`uninstCat_${c}` as TKey)} ({counts[c]})
              </button>
            ))}
            <span className="muted" style={{ fontSize: 11, alignSelf: "center" }}>{t("uninstCatHint")}</span>
          </div>
        )}

        {loading ? (
          <div className="row" style={{ gap: 10 }}><Spinner /><span className="muted">{t("uninstScanning")}</span></div>
        ) : loadErr ? (
          <div style={{ color: "var(--red)", marginBottom: 10 }}>{loadErr}</div>
        ) : filtered.length === 0 ? (
          <div className="muted">{t("uninstNoApps")}{search || category ? ` ${t("uninstMatchingFilter")}` : ""}.</div>
        ) : (
          <div style={{ maxHeight: "65vh", overflowY: "auto" }}>
            {filtered.map((app: any) => {
              const key = appKey(app);
              const st = status[key];
              const appLog = log[app.name];
              const appLeft = leftovers[app.name] ?? [];
              const appSel = selLeft[app.name] ?? new Set<string>();
              const wasLaunched = st?.state === "launched";

              return (
                <div key={key} style={{ borderBottom: "1px solid var(--border)", padding: "8px 0", opacity: app.removed ? 0.75 : 1 }}>
                  <div className="row" style={{ alignItems: "center", gap: 10 }}>
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ fontWeight: 600, fontSize: 13 }}>{app.name}</div>
                      <div className="muted" style={{ fontSize: 11 }}>
                        {app.publisher && <span>{app.publisher}</span>}
                        {app.version && <span> · v{app.version}</span>}
                        {app.sizeMb > 0 && <span> · {fmtMb(app.sizeMb)}</span>}
                        {/^\d{8}$/.test(app.installDate ?? "") && <span> · {app.installDate.slice(6, 8)}.{app.installDate.slice(4, 6)}.{app.installDate.slice(0, 4)}</span>}
                        {!category && app.category !== "other" && <span> · {t(`uninstCat_${app.category}` as TKey)}</span>}
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
                        disabled={busy !== null || !app.uninstallString}
                      >
                        {busy === key ? "…" : t("uninstUninstallBtn")}
                      </button>
                    )}
                  </div>

                  {st && (
                    <div className="mono" style={{ fontSize: 11, marginTop: 4, color: st.state === "error" ? "var(--red)" : "var(--muted)" }}>{st.msg}</div>
                  )}
                  {appLog && (
                    <div className="mono muted" style={{ fontSize: 11, marginTop: 4 }}>{appLog}</div>
                  )}

                  {appLeft.length > 0 && (
                    <div style={{ marginTop: 8, padding: 8, background: "var(--bg2)", borderRadius: 4, border: "1px solid var(--border)" }}>
                      <div className="row" style={{ marginBottom: 6, alignItems: "center", gap: 8 }}>
                        <span style={{ fontSize: 12, fontWeight: 600 }}>{appLeft.length} {t("uninstLeftoversFound")}</span>
                        <div style={{ flex: 1 }} />
                        {confirmClean !== app.name ? (
                          <button
                            className="btn small danger"
                            onClick={() => setConfirmClean(app.name)}
                            disabled={!appSel.size || cleaning === app.name}
                          >
                            {cleaning === app.name ? "…" : `🗑 ${t("uninstClean")} (${appSel.size})`}
                          </button>
                        ) : (
                          <>
                            <span style={{ color: "var(--yellow)", fontSize: 12 }}>⚠ {t("uninstConfirmClean")} ({appSel.size})</span>
                            <button className="btn small danger" onClick={() => cleanLeft(app.name)} disabled={cleaning === app.name}>
                              {cleaning === app.name ? "…" : t("uninstConfirmCleanBtn")}
                            </button>
                            <button className="btn small ghost" onClick={() => setConfirmClean(null)} disabled={cleaning === app.name}>{t("cancel")}</button>
                          </>
                        )}
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
