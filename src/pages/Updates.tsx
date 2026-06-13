import React, { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";

type AppRow = {
  name: string; id: string; version: string; available: string;
  source: string; publisher: string; location: string; targetable: boolean;
};
type RowStatus = { state: "idle" | "updating" | "done" | "error"; msg?: string };

export default function Updates({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [apps, setApps]           = useState<AppRow[]>([]);
  const [appsMeta, setAppsMeta]   = useState<{ count: number } | null>(null);
  const [scanBusy, setScanBusy]   = useState(false);
  const [scanErr, setScanErr]     = useState("");
  const [rowStatus, setRowStatus] = useState<Record<string, RowStatus>>({});
  const [allBusy, setAllBusy]     = useState(false);
  const [allLog, setAllLog]       = useState<string[]>([]);
  const logRef = useRef<HTMLDivElement>(null);
  const [drivers, setDrivers]     = useState<any | null>(null);
  const [drvBusy, setDrvBusy]     = useState(false);
  const [drvResult, setDrvResult] = useState<any | null>(null);
  const [confirmDrv, setConfirmDrv] = useState(false);
  const [gpu, setGpu]             = useState<any | null>(null);

  const pushLog = (msg: string) => {
    setAllLog(l => [...l, `[${new Date().toLocaleTimeString()}] ${msg}`]);
    setTimeout(() => logRef.current?.scrollTo({ top: 9999, behavior: "smooth" }), 50);
  };
  const setRow = (id: string, s: RowStatus) => setRowStatus(prev => ({ ...prev, [id]: s }));

  const scanApps = async () => {
    setScanBusy(true); setScanErr(""); setRowStatus({});
    try {
      const r: any = await api.scanAppUpdates();
      setApps(r.apps ?? []); setAppsMeta({ count: r.count ?? 0 });
    } catch (e: any) { setScanErr(String(e)); setApps([]); }
    finally { setScanBusy(false); }
  };

  useEffect(() => { scanApps(); api.gpuVendor().then(setGpu).catch(() => {}); }, []);

  const updateOne = async (a: AppRow) => {
    setRow(a.id, { state: "updating" });
    pushLog(`▶ Updating ${a.name} (${a.id})…`);
    try {
      const r: any = await api.updateApps(a.id);
      const lastLine = (r.log ?? "").split("\n").filter(Boolean).pop() ?? "done";
      setRow(a.id, { state: "done", msg: lastLine });
      pushLog(`✔ ${a.name}: ${lastLine}`);
    } catch (e: any) {
      const msg = String(e);
      setRow(a.id, { state: "error", msg });
      pushLog(`✘ ${a.name}: ${msg}`);
    }
  };

  const updateAll = async () => {
    setAllBusy(true); setAllLog([]);
    pushLog("▶ winget upgrade --all…");
    try {
      const r: any = await api.updateApps();
      (r.log ?? "").split("\n").filter(Boolean).forEach((l: string) => pushLog(l));
      pushLog(`✔ ${t("updatesAllDone")}`);
      await scanApps();
    } catch (e: any) { pushLog(`✘ ${e}`); }
    finally { setAllBusy(false); }
  };

  const scanDrivers = async () => {
    setDrvBusy(true); setDrivers(null); setDrvResult(null);
    try { setDrivers(await api.scanDriverUpdates()); }
    finally { setDrvBusy(false); }
  };

  const anyRowUpdating = Object.values(rowStatus).some(s => s.state === "updating");
  const doneCount      = Object.values(rowStatus).filter(s => s.state === "done").length;
  const updatingCount  = Object.values(rowStatus).filter(s => s.state === "updating").length;

  return (
    <>
      <div className="page-title">{t("updatesTitle")}</div>
      <div className="page-sub">{t("updatesSub")}</div>

      {/* App Updates */}
      <Card title={`${t("updatesAppTitle")} ${appsMeta != null ? `(${appsMeta.count})` : ""}`}>

        {/* Status banner */}
        {(allBusy || anyRowUpdating) && (
          <div style={{
            background: "var(--accent)11", border: "1px solid var(--accent)44",
            borderRadius: 8, padding: "10px 14px", marginBottom: 12,
            display: "flex", alignItems: "center", gap: 10,
          }}>
            <Spinner />
            <span style={{ fontWeight: 600 }}>
              {allBusy ? t("updatesUpdatingAll") : `${updatingCount} ${t("updatesUpdatingN")}`}
            </span>
            <span className="muted" style={{ fontSize: 12 }}>{t("updatesTakesTime")}</span>
          </div>
        )}
        {doneCount > 0 && !anyRowUpdating && !allBusy && (
          <div style={{ color: "var(--green)", marginBottom: 10, fontWeight: 600, fontSize: 13 }}>
            ✔ {doneCount} {t("updatesDoneN")}
          </div>
        )}

        {scanBusy && <><Spinner /> <span className="muted">{t("updatesScanning")}</span></>}
        {scanErr && <div style={{ color: "var(--yellow)", marginBottom: 8 }}>{scanErr}</div>}
        {!scanBusy && apps.length === 0 && appsMeta && (
          <div className="muted">{t("updatesAllCurrent")}</div>
        )}

        {apps.length > 0 && (
          <>
            <table className="tbl">
              <thead>
                <tr>
                  <th>{t("colApp")}</th>
                  <th>{t("colInstalled")}</th>
                  <th>{t("colAvailable")}</th>
                  <th>{t("colSource")}</th>
                  <th>{t("colPath")}</th>
                  <th style={{ width: 160 }}></th>
                </tr>
              </thead>
              <tbody>
                {apps.map((a) => {
                  const rs = rowStatus[a.id] ?? { state: "idle" };
                  return (
                    <tr key={a.id}>
                      <td>
                        <div style={{ fontWeight: 500 }}>{a.name}</div>
                        {a.publisher && <div className="muted" style={{ fontSize: 11 }}>{a.publisher}</div>}
                        <div className="mono muted" style={{ fontSize: 10 }}>{a.id}</div>
                      </td>
                      <td className="muted">{a.version}</td>
                      <td style={{ color: "var(--cyan)", fontWeight: 600 }}>{a.available}</td>
                      <td className="muted">{a.source || "—"}</td>
                      <td style={{ maxWidth: 180 }}>
                        {a.location ? (
                          <span
                            className="mono"
                            style={{ color: "var(--accent)", cursor: "pointer", fontSize: 10, wordBreak: "break-all" }}
                            title={a.location}
                            onClick={() => api.openPath(a.location)}
                          >
                            📂 {a.location}
                          </span>
                        ) : (
                          <span className="muted" style={{ fontSize: 11 }}>
                            {a.source === "msstore" ? "Store" : "—"}
                          </span>
                        )}
                      </td>
                      <td>
                        {rs.state === "idle" && (
                          a.targetable ? (
                            <button className="btn small" disabled={anyRowUpdating || allBusy} onClick={() => updateOne(a)}>
                              ⟳ Update
                            </button>
                          ) : (
                            <span className="muted" style={{ fontSize: 11 }}>{t("updatesViaAll")}</span>
                          )
                        )}
                        {rs.state === "updating" && (
                          <span style={{ display: "flex", alignItems: "center", gap: 6 }}>
                            <Spinner /><span className="muted" style={{ fontSize: 11 }}>{t("updatesRowRunning")}</span>
                          </span>
                        )}
                        {rs.state === "done" && (
                          <span style={{ color: "var(--green)", fontSize: 12 }}>{t("updatesRowDone")}</span>
                        )}
                        {rs.state === "error" && (
                          <span style={{ color: "var(--red)", fontSize: 11 }} title={rs.msg}>{t("updatesRowError")}</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>

            <div style={{ display: "flex", gap: 10, marginTop: 12, alignItems: "center", flexWrap: "wrap" }}>
              <button className="btn" disabled={allBusy || anyRowUpdating} onClick={updateAll}>
                {allBusy ? <><Spinner /> {t("updatesUpdatingAll")}</> : `⟳ ${t("updatesAllBtn")} (${apps.length})`}
              </button>
              <button className="btn ghost" onClick={scanApps} disabled={scanBusy || allBusy || anyRowUpdating}>
                {t("rescan")}
              </button>
              {!admin && <span className="muted" style={{ fontSize: 12 }}>{t("updatesAdminHint")}</span>}
            </div>

            {allLog.length > 0 && (
              <div ref={logRef} className="mono muted" style={{
                marginTop: 12, background: "var(--card2)",
                border: "1px solid var(--border)", borderRadius: 6,
                padding: "10px 12px", fontSize: 11, lineHeight: 1.7,
                maxHeight: 180, overflowY: "auto",
              }}>
                {allLog.map((l, i) => (
                  <div key={i} style={{ color: l.includes("✔") ? "var(--green)" : l.includes("✘") ? "var(--red)" : undefined }}>{l}</div>
                ))}
              </div>
            )}
          </>
        )}
      </Card>

      {/* Driver Updates */}
      <div className="mt">
        <Card title={`${t("updatesDriverTitle")}${drivers?.count != null ? ` (${drivers.count})` : ""}`}>
          {!drivers && !drvBusy && (
            <button className="btn ghost" onClick={scanDrivers}>{t("updatesCheckDrivers")}</button>
          )}
          {drvBusy && <><Spinner /> <span className="muted">{t("updatesDriverScanning")}</span></>}
          {drivers?.error && <div style={{ color: "var(--yellow)" }}>{String(drivers.error)}</div>}
          {drivers?.count === 0 && <div className="muted">{t("updatesNoDrivers")}</div>}
          {drivers?.count > 0 && (
            <>
              <table className="tbl">
                <thead><tr><th>{t("colDriver")}</th><th>{t("colManufacturer")}</th><th>{t("colDate")}</th><th>{t("colSize")}</th></tr></thead>
                <tbody>
                  {drivers.drivers.map((d: any, i: number) => (
                    <tr key={i}>
                      <td>{d.title}</td>
                      <td className="muted">{d.manufacturer}</td>
                      <td className="muted">{d.verDate}</td>
                      <td className="muted">{d.sizeMb} MB</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <div style={{ display: "flex", gap: 10, marginTop: 12, flexWrap: "wrap", alignItems: "center" }}>
                {!confirmDrv ? (
                  <button className="btn" disabled={!admin} onClick={() => setConfirmDrv(true)}>
                    {drivers.count} {t("updatesDriverInstallBtn")}
                  </button>
                ) : (
                  <>
                    <span style={{ color: "var(--yellow)", fontSize: 13 }}>
                      {t("updatesDriverConfirm")} {drivers.count} {t("updatesDriverQuestion")}
                    </span>
                    <button className="btn ghost" onClick={async () => {
                      try { await api.createRestorePoint("Before driver updates"); } catch {}
                    }}>{t("updatesRestorePoint")}</button>
                    <button className="btn" style={{ background: "var(--red)" }} onClick={async () => {
                      try { setDrvResult(await api.installDriverUpdates()); }
                      catch (e: any) { setDrvResult({ error: String(e) }); }
                      setConfirmDrv(false);
                    }}>{t("install")}</button>
                    <button className="btn ghost" onClick={() => setConfirmDrv(false)}>{t("cancel")}</button>
                  </>
                )}
                {!admin && <span className="muted" style={{ fontSize: 12 }}>{t("updatesAdminNeeded")}</span>}
              </div>
            </>
          )}
          {drvResult && !drvResult.error && (
            <div style={{ marginTop: 10 }}>
              <div style={{ color: "var(--green)", fontWeight: 600 }}>✔ {drvResult.overall} — {drvResult.installed} drivers</div>
              {(drvResult.results ?? []).map((r: any, i: number) => (
                <div key={i} className="mono muted" style={{ fontSize: 11 }}>{r.result} · {r.title}</div>
              ))}
              {drvResult.rebootRequired && (
                <div style={{ color: "var(--yellow)", fontWeight: 600, marginTop: 8 }}>{t("updatesReboot")}</div>
              )}
            </div>
          )}
          {drvResult?.error && <div style={{ color: "var(--red)", marginTop: 8 }}>{drvResult.error}</div>}
        </Card>
      </div>

      {/* GPU driver hint */}
      {gpu?.vendor && gpu.vendor !== "unknown" && (
        <div className="mt">
          <Card title={t("updatesGpuTitle")}>
            <div className="muted" style={{ marginBottom: 10 }}>
              {t("updatesGpuNote")} <b style={{ color: "var(--text)" }}>{gpu.gpus}</b> {t("updatesGpuNote2")}
            </div>
            <button className="btn ghost small" onClick={() => api.openPath(gpu.url)}>
              {gpu.vendor} {t("updatesGpuOpen")}
            </button>
          </Card>
        </div>
      )}
    </>
  );
}
