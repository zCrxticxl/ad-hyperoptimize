import React, { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Card, Spinner, Badge } from "../components/ui";
import { useLang } from "../i18n";
import { useFeatureFocus } from "../hooks/useFeatureFocus";

type Service = {
  name: string; displayName: string; status: string;
  startType: string; description: string; isBloat: boolean; bloatReason: string;
  origin: "windows" | "thirdparty";
};

const STATUS_CLS: Record<string, string> = {
  Running: "st-applied", Stopped: "st-unknown", Paused: "st-medium",
};
const START_OPTIONS = ["Automatic", "AutomaticDelayedStart", "Manual", "Disabled"];

export default function ServicesManager({ admin, focusId }: { admin: boolean; focusId?: string }) {
  const { t } = useLang();
  const [services, setServices] = useState<Service[]>([]);
  const [loading, setLoading]   = useState(true);
  const [tab, setTab]           = useState<"bloat" | "third" | "all">("bloat");
  const [search, setSearch]     = useState("");
  const [busy, setBusy]         = useState<string | null>(null);
  const [err, setErr]           = useState("");
  const [log, setLog]           = useState<string[]>([]);
  useFeatureFocus(focusId, services.length > 0);

  const push = (m: string) =>
    setLog(l => [`[${new Date().toLocaleTimeString()}] ${m}`, ...l.slice(0, 99)]);

  const refresh = () => {
    setLoading(true);
    api.servicesList()
      .then((r: any) => setServices(r.services ?? []))
      .catch(e => setErr(String(e)))
      .finally(() => setLoading(false));
  };
  useEffect(() => { refresh(); }, []);

  const visible = useMemo(() => {
    let list = tab === "bloat" ? services.filter(s => s.isBloat)
      : tab === "third" ? services.filter(s => s.origin === "thirdparty")
      : services;
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter(s =>
        s.name.toLowerCase().includes(q) ||
        s.displayName.toLowerCase().includes(q) ||
        s.description.toLowerCase().includes(q)
      );
    }
    return list;
  }, [services, tab, search]);

  const bloatCount  = useMemo(() => services.filter(s => s.isBloat).length, [services]);
  const thirdCount  = useMemo(() => services.filter(s => s.origin === "thirdparty").length, [services]);
  const runCount    = useMemo(() => services.filter(s => s.status === "Running").length, [services]);
  const bloatActive = useMemo(() => services.filter(s => s.isBloat && s.status === "Running").length, [services]);

  const doSetStartup = async (svc: Service, startupType: string, refreshAfter = true) => {
    setBusy(svc.name + "_startup");
    push(`${svc.displayName}: → ${startupType}…`);
    try {
      await api.serviceSetStartup(svc.name, startupType);
      push(`✔ ${svc.displayName}: ${startupType}`);
    } catch (e: any) { push(`✘ ${svc.displayName}: ${e}`); setErr(String(e)); }
    finally { setBusy(null); if (refreshAfter) refresh(); }
  };

  const doControl = async (svc: Service, action: string) => {
    setBusy(svc.name + "_ctrl");
    push(`${svc.displayName}: ${action}…`);
    try {
      await api.serviceControl(svc.name, action);
      push(`✔ ${svc.displayName}: ${action} OK`);
    } catch (e: any) { push(`✘ ${svc.displayName}: ${e}`); setErr(String(e)); }
    finally { setBusy(null); refresh(); }
  };

  const disableBloat = async () => {
    for (const s of services.filter(s => s.isBloat && s.startType !== "Disabled")) {
      await doSetStartup(s, "Disabled", false);
    }
    refresh();
  };

  return (
    <>
      <h1 className="page-title">{t("svcTitle")}</h1>
      <div className="page-sub">{t("svcSub")}</div>

      {!admin && <div className="warn-banner">{t("svcAdminWarn")}</div>}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}

      {!loading && (
        <div className="stat-row" style={{ marginBottom: 12 }}>
          <div className="stat-card"><div className="stat-val">{services.length}</div><div className="stat-lbl">{t("svcTotal")}</div></div>
          <div className="stat-card"><div className="stat-val" style={{ color: "var(--green)" }}>{runCount}</div><div className="stat-lbl">{t("svcRunning")}</div></div>
          <div className="stat-card"><div className="stat-val" style={{ color: "var(--yellow)" }}>{bloatCount}</div><div className="stat-lbl">{t("svcBloat")}</div></div>
          <div className="stat-card"><div className="stat-val" style={{ color: "var(--accent)" }}>{thirdCount}</div><div className="stat-lbl">{t("schedThird")}</div></div>
          <div className="stat-card"><div className="stat-val" style={{ color: bloatActive > 0 ? "var(--red)" : "var(--green)" }}>{bloatActive}</div><div className="stat-lbl">{t("svcBloatActive")}</div></div>
        </div>
      )}

      <div style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 12, flexWrap: "wrap" }}>
        <button className={`btn small ${tab === "bloat" ? "" : "ghost"}`} onClick={() => setTab("bloat")}>
          {t("svcBloatTab")} ({bloatCount})
        </button>
        <button className={`btn small ${tab === "third" ? "" : "ghost"}`} onClick={() => setTab("third")}>
          {t("schedThird")} ({thirdCount})
        </button>
        <button className={`btn small ${tab === "all" ? "" : "ghost"}`} onClick={() => setTab("all")}>
          {t("svcAllTab")} ({services.length})
        </button>
        <input
          className="search-input"
          placeholder={t("search")}
          value={search}
          onChange={e => setSearch(e.target.value)}
          style={{ flex: 1, minWidth: 160 }}
        />
        {tab === "bloat" && (
          <button className="btn small" disabled={!!busy || !admin} onClick={disableBloat}>
            {t("svcDisableAll")}
          </button>
        )}
        <button className="btn ghost small" disabled={loading || !!busy} onClick={refresh}>{t("refresh")}</button>
      </div>

      {loading && <><Spinner /> <span className="muted">{t("svcLoading")}</span></>}

      {!loading && (
        <Card title={`${t(tab === "bloat" ? "svcBloatTab" : tab === "third" ? "schedThird" : "svcAllTab")} · ${visible.length}`}>
          <table className="tbl svc-tbl">
            <tbody>
              {visible.map(svc => {
                const isBusy = busy === svc.name + "_startup" || busy === svc.name + "_ctrl";
                const running = svc.status === "Running";
                const tip = [svc.description, svc.bloatReason].filter(Boolean).join("\n");
                return (
                  <tr key={svc.name} data-focus-id={svc.name} style={{ opacity: svc.status === "Stopped" ? 0.72 : 1 }}>
                    <td style={{ maxWidth: 0 }}>
                      <div className="svc-cell-name" title={tip}>
                        <span className="svc-name">{svc.displayName}</span>
                        <span className="muted svc-sname">({svc.name})</span>
                      </div>
                      {svc.isBloat && (
                        <span className="svc-badge risk-Medium" title={svc.bloatReason}>Bloat</span>
                      )}
                      {svc.origin === "thirdparty" && (
                        <span className="svc-badge svc-third" title={t("schedThird")}>{t("schedThird")}</span>
                      )}
                      <Badge cls={STATUS_CLS[svc.status] ?? "st-unknown"}>{svc.status}</Badge>
                    </td>
                    <td className="svc-cell-ctl">
                      <select
                        className="sel"
                        value={svc.startType}
                        disabled={!!busy || isBusy || !admin}
                        onChange={e => doSetStartup(svc, e.target.value)}
                        title={svc.startType}
                      >
                        {START_OPTIONS.map(o => <option key={o} value={o}>{o}</option>)}
                      </select>
                      {running ? (
                        <button className="btn small ghost" disabled={!!busy || isBusy || !admin} onClick={() => doControl(svc, "stop")}>
                          {isBusy ? <Spinner /> : t("svcStop")}
                        </button>
                      ) : (
                        <button className="btn small" disabled={!!busy || isBusy || !admin || svc.startType === "Disabled"} onClick={() => doControl(svc, "start")}>
                          {isBusy ? <Spinner /> : t("svcStart")}
                        </button>
                      )}
                      <button className="btn small ghost" disabled={!!busy || isBusy || !admin || !running} onClick={() => doControl(svc, "restart")} title="↺">
                        {isBusy ? <Spinner /> : "↺"}
                      </button>
                    </td>
                  </tr>
                );
              })}
              {visible.length === 0 && (
                <tr><td colSpan={2} className="muted" style={{ textAlign: "center", padding: "20px 0" }}>
                  {search ? t("svcNoResults") : tab === "bloat" ? t("svcNoBloat") : "—"}
                </td></tr>
              )}
            </tbody>
          </table>
        </Card>
      )}

      {log.length > 0 && (
        <div className="mt">
          <Card title={t("log")}>
            <div className="mono muted" style={{ fontSize: 11, lineHeight: 1.8, maxHeight: 160, overflowY: "auto" }}>
              {log.map((l, i) => (
                <div key={i} style={{ color: l.includes("✔") ? "var(--green)" : l.includes("✘") ? "var(--red)" : undefined }}>{l}</div>
              ))}
            </div>
          </Card>
        </div>
      )}
    </>
  );
}
