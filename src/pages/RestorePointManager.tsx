import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";

type RP = {
  SequenceNumber: number;
  Description: string;
  RestorePointType: number | string;
  CreationTime: string;
};

const RP_TYPES: Record<number, string> = {
  0:  "Application Install",
  1:  "Application Uninstall",
  6:  "Restore Operation",
  7:  "Checkpoint",
  10: "Device Driver Install",
  12: "Modify Settings",
  13: "Cancelled Operation",
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
  const [points, setPoints] = useState<RP[]>([]);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy]     = useState<number | null>(null);
  const [creating, setCreating] = useState(false);
  const [desc, setDesc]     = useState("AD HyperOptimize Checkpoint");
  const [log, setLog]       = useState("");
  const [confirm, setConfirm] = useState<number | null>(null);

  const load = async () => {
    setLoading(true);
    setLog("");
    try {
      const r = await api.listRestorePoints();
      const arr = Array.isArray(r) ? r : r?.error ? [] : [r];
      setPoints(arr.sort((a: RP, b: RP) => b.SequenceNumber - a.SequenceNumber));
      if (r?.error) setLog(`Error: ${r.error}`);
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
      <div className="page-title">🛟 Restore Points</div>
      <div className="page-sub">
        Create and manage System Restore Points. Use "System Restore" to roll back the OS to a checkpoint.
        {!admin && <span style={{ color: "var(--orange)" }}> · Admin required to create/delete restore points.</span>}
      </div>

      {/* Create */}
      <Card title="Create Restore Point" style={{ marginBottom: 14 }}>
        <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
          <input
            value={desc}
            onChange={e => setDesc(e.target.value)}
            placeholder="Description…"
            style={{ flex: 1, minWidth: 200, padding: "6px 10px", fontSize: 13, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}
          />
          <button className="btn" disabled={creating || !desc.trim() || !admin} onClick={create}>
            {creating ? <><Spinner /> Creating…</> : "🛟 Create"}
          </button>
          <button className="btn ghost" onClick={openRstrui}>
            🪟 System Restore…
          </button>
        </div>
        {!admin && (
          <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>Restart the app as administrator to create restore points.</div>
        )}
      </Card>

      {/* List */}
      <Card title={`Restore Points (${points.length})`}>
        <div className="row" style={{ gap: 8, marginBottom: 12 }}>
          <button className="btn small ghost" onClick={load} disabled={loading}>↺ Refresh</button>
        </div>

        {loading ? (
          <div className="row" style={{ gap: 10 }}><Spinner /><span className="muted">Loading…</span></div>
        ) : points.length === 0 ? (
          <div className="muted">No restore points found. Create one above, or check if System Restore is enabled.</div>
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
                    <div style={{ fontWeight: 600, fontSize: 13 }}>{rp.Description || "(no description)"}</div>
                    <div className="muted" style={{ fontSize: 11, marginTop: 2 }}>
                      {fmtDate(rp.CreationTime)}
                      {rp.RestorePointType !== undefined && (
                        <span> · {RP_TYPES[Number(rp.RestorePointType)] ?? `Type ${rp.RestorePointType}`}</span>
                      )}
                    </div>
                  </div>
                  <div className="row" style={{ gap: 6, flexShrink: 0 }}>
                    {confirm === rp.SequenceNumber ? (
                      <>
                        <span className="muted" style={{ fontSize: 12, alignSelf: "center" }}>Delete?</span>
                        <button className="btn small danger" disabled={busy === rp.SequenceNumber} onClick={() => del(rp.SequenceNumber)}>
                          {busy === rp.SequenceNumber ? <><Spinner /></> : "Yes"}
                        </button>
                        <button className="btn small ghost" onClick={() => setConfirm(null)}>No</button>
                      </>
                    ) : (
                      <button
                        className="btn small ghost danger"
                        disabled={!admin || busy !== null}
                        onClick={() => setConfirm(rp.SequenceNumber)}
                      >
                        🗑 Delete
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
          "System Restore" opens the Windows restore wizard where you can roll back to any checkpoint.
          Deleting a restore point is permanent. Windows auto-creates points when tweaks are applied.
        </div>
      </Card>
    </>
  );
}
