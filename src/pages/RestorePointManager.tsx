import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";

type RP = {
  SequenceNumber: number;
  Description: string;
  RestorePointType: number | string;
  CreationTime: string;
};

function fmtDate(raw: string): string {
  // WMI date: "20250613120000.000000+000" or ISO
  if (raw.match(/^\d{14}/)) {
    const y = raw.slice(0, 4), mo = raw.slice(4, 6), d = raw.slice(6, 8);
    const h = raw.slice(8, 10), mi = raw.slice(10, 12);
    return `${d}.${mo}.${y}  ${h}:${mi}`;
  }
  try { return new Date(raw).toLocaleString(); } catch { return raw; }
}

export default function RestorePointManager({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [points, setPoints] = useState<RP[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy]     = useState<number | null>(null);
  const [creating, setCreating] = useState(false);
  const [desc, setDesc]     = useState("AD HyperOptimize Checkpoint");
  const [log, setLog]       = useState("");
  const [confirm, setConfirm] = useState<number | null>(null);

  const RP_TYPES: Record<number, string> = {
    0:  t("restoreTypeAppInstall"),
    1:  t("restoreTypeAppUninstall"),
    6:  t("restoreTypeRestoreOp"),
    7:  t("restoreTypeCheckpoint"),
    10: t("restoreTypeDriverInstall"),
    12: t("restoreTypeModifySettings"),
    13: t("restoreTypeCancelledOp"),
  };

  const load = async () => {
    setLoading(true);
    setLog("");
    try {
      const r = await api.listRestorePoints();
      const arr = Array.isArray(r) ? r : r?.error ? [] : [r];
      setPoints(arr.sort((a: RP, b: RP) => b.SequenceNumber - a.SequenceNumber));
      if (r?.error) setLog(`${t("error")}: ${r.error}`);
    } finally { setLoading(false); }
  };

  useEffect(() => { load(); }, []);

  const create = async () => {
    if (!desc.trim()) return;
    setCreating(true);
    setLog("");
    try {
      const r = await api.createRestorePoint(desc);
      setLog(r);
      await load();
    } catch (e: any) { setLog(String(e)); }
    finally { setCreating(false); }
  };

  const del = async (seq: number) => {
    setBusy(seq);
    setLog("");
    try {
      const r = await api.deleteRestorePoint(seq);
      setLog(r);
      await load();
    } catch (e: any) { setLog(String(e)); }
    finally { setBusy(null); setConfirm(null); }
  };

  const openRstrui = async () => {
    try { setLog(await api.launchRstrui()); }
    catch (e: any) { setLog(String(e)); }
  };

  return (
    <>
      <div className="page-title">🛟 {t("restoreTitle")}</div>
      <div className="page-sub">
        {t("restoreSub")}
        {!admin && <span style={{ color: "var(--orange)" }}> · {t("restoreAdminRequiredHint")}</span>}
      </div>

      {/* Create */}
      <Card title={t("restoreCreateTitle")} style={{ marginBottom: 14 }}>
        <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
          <input
            value={desc}
            onChange={e => setDesc(e.target.value)}
            placeholder={t("restoreDescPlaceholder")}
            style={{ flex: 1, minWidth: 200, padding: "6px 10px", fontSize: 13, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}
          />
          <button className="btn" disabled={creating || !desc.trim() || !admin} onClick={create}>
            {creating ? <><Spinner /> {t("restoreCreating")}</> : `🛟 ${t("restoreCreateBtn")}`}
          </button>
          <button className="btn ghost" onClick={openRstrui}>
            🪟 {t("restoreSystemRestoreBtn")}
          </button>
        </div>
        {!admin && (
          <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>{t("restoreAdminHint")}</div>
        )}
      </Card>

      {/* List */}
      <Card title={`${t("restorePointsTitle")} (${points.length})`}>
        <div className="row" style={{ gap: 8, marginBottom: 12 }}>
          <button className="btn small ghost" onClick={load} disabled={loading}>↺ {t("refresh2")}</button>
        </div>

        {loading ? (
          <div className="row" style={{ gap: 10 }}><Spinner /><span className="muted">{t("loading")}</span></div>
        ) : points.length === 0 ? (
          <div className="muted">{t("restoreEmpty")}</div>
        ) : (
          <div style={{ maxHeight: "55vh", overflowY: "auto" }}>
            {points.map((rp) => (
              <div key={rp.SequenceNumber} style={{ borderBottom: "1px solid var(--border)", padding: "10px 0" }}>
                <div className="row" style={{ gap: 10, alignItems: "center", flexWrap: "wrap" }}>
                  <div style={{ minWidth: 36, textAlign: "center" }}>
                    <div style={{ fontSize: 18 }}>🛟</div>
                    <div className="muted" style={{ fontSize: 10 }}>#{rp.SequenceNumber}</div>
                  </div>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontWeight: 600, fontSize: 13 }}>{rp.Description || t("restoreNoDescription")}</div>
                    <div className="muted" style={{ fontSize: 11, marginTop: 2 }}>
                      {fmtDate(rp.CreationTime)}
                      {rp.RestorePointType !== undefined && (
                        <span> · {RP_TYPES[Number(rp.RestorePointType)] ?? `${t("restoreTypeGeneric")} ${rp.RestorePointType}`}</span>
                      )}
                    </div>
                  </div>
                  <div className="row" style={{ gap: 6, flexShrink: 0 }}>
                    {confirm === rp.SequenceNumber ? (
                      <>
                        <span className="muted" style={{ fontSize: 12, alignSelf: "center" }}>{t("restoreDeleteConfirm")}</span>
                        <button className="btn small danger" disabled={busy === rp.SequenceNumber} onClick={() => del(rp.SequenceNumber)}>
                          {busy === rp.SequenceNumber ? <><Spinner /></> : t("restoreYes")}
                        </button>
                        <button className="btn small ghost" onClick={() => setConfirm(null)}>{t("restoreNo")}</button>
                      </>
                    ) : (
                      <button
                        className="btn small ghost danger"
                        disabled={!admin || busy !== null}
                        onClick={() => setConfirm(rp.SequenceNumber)}
                      >
                        🗑 {t("restoreDeleteBtn")}
                      </button>
                    )}
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}

        {log && (
          <div className="muted" style={{ fontSize: 12, marginTop: 10 }}>{log}</div>
        )}

        <div className="muted" style={{ fontSize: 11, marginTop: 12 }}>
          {t("restoreFootSystemRestore")}
          {t("restoreFootDeletePermanent")}
        </div>
      </Card>
    </>
  );
}
