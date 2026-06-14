import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { HwWarnings } from "../components/HwWarnings";

const PLAN_DESC: Record<string, string> = {
  "381b4222-f694-41f0-9685-ff5bb260df2e": "Balances performance with energy — Windows default.",
  "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c": "Favors performance over energy saving. No CPU throttling.",
  "a1841308-3541-4fab-bc81-f71556f20b4a": "Reduces PC performance to save energy. Not for gaming.",
  "e9a42b02-d5df-448d-aa00-03f14749eb61": "Eliminates micro-latency by preventing CPU from downclocking. Best for competitive gaming.",
};

export default function PowerPlan({ admin }: { admin: boolean }) {
  const [data, setData] = useState<any>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [log, setLog] = useState("");
  const [newName, setNewName] = useState("");
  const [showCreate, setShowCreate] = useState(false);

  const load = async () => setData(await api.powerplanList());
  useEffect(() => { load(); }, []);

  const act = async (label: string, fn: () => Promise<string>) => {
    setBusy(label);
    try { setLog(await fn()); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(null); }
  };

  const plans: any[] = data?.plans ?? [];
  const hasUltimate = data?.ultimateAvailable ?? false;
  const ultimateGuid = data?.ultimateGuid ?? "";

  const activeGuid = plans.find((p: any) => p.active)?.guid ?? "";

  return (
    <>
      <div className="page-title">🔋 Power Plan Manager</div>
      <div className="page-sub">
        Switch Windows power schemes, unlock hidden Ultimate Performance plan, create custom plans.
        {!admin && <span style={{ color: "var(--orange)" }}> · Admin required to change plans.</span>}
      </div>
      <HwWarnings page="power_plan" />

      {!hasUltimate && (
        <Card title="⚡ Unlock Ultimate Performance" style={{ marginBottom: 14 }}>
          <div className="muted" style={{ fontSize: 13, marginBottom: 10 }}>
            Ultimate Performance is a hidden plan built into Windows 10/11 Pro that eliminates
            micro-latency by keeping the CPU at max frequency with no power-saving downclocks.
            Ideal for competitive gaming and low-latency workloads.
          </div>
          <button
            className="btn"
            onClick={() => act("unlock", api.powerplanUnlockUltimate)}
            disabled={busy !== null || !admin}
          >
            {busy === "unlock" ? <><Spinner /> Unlocking…</> : "⚡ Unlock Ultimate Performance"}
          </button>
          {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 8 }}>{log}</div>}
        </Card>
      )}

      <Card title={`Power Plans (${plans.length})`}>
        {!data ? <Spinner /> : (
          <>
            {plans.map((p: any) => (
              <div
                key={p.guid}
                className="row"
                style={{
                  padding: "12px 0",
                  borderBottom: "1px solid var(--border)",
                  alignItems: "flex-start",
                  gap: 12,
                }}
              >
                <div style={{ flex: 1 }}>
                  <div className="row" style={{ alignItems: "center", gap: 8, marginBottom: 3 }}>
                    {p.active && <span style={{ color: "var(--green)", fontWeight: 700 }}>●</span>}
                    <b style={{ color: p.active ? "var(--accent)" : "var(--fg)" }}>{p.name}</b>
                    {p.active && <span style={{ fontSize: 11, color: "var(--green)" }}>ACTIVE</span>}
                  </div>
                  <div className="muted" style={{ fontSize: 12 }}>
                    {PLAN_DESC[p.guid.toLowerCase()] ?? "Custom power plan"}
                  </div>
                  <div className="mono muted" style={{ fontSize: 10, marginTop: 2 }}>{p.guid}</div>
                </div>
                <div className="row" style={{ gap: 6, flexShrink: 0 }}>
                  {!p.active && (
                    <button
                      className="btn small"
                      onClick={() => act(p.guid, () => api.powerplanSet(p.guid))}
                      disabled={busy !== null || !admin}
                    >
                      {busy === p.guid ? "…" : "Set Active"}
                    </button>
                  )}
                  {!["381b4222-f694-41f0-9685-ff5bb260df2e",
                     "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c",
                     "a1841308-3541-4fab-bc81-f71556f20b4a",
                     "e9a42b02-d5df-448d-aa00-03f14749eb61"].includes(p.guid.toLowerCase()) && (
                    <button
                      className="btn small ghost danger"
                      onClick={async () => {
                        if (!window.confirm(`Delete plan "${p.name}"?`)) return;
                        act(`del-${p.guid}`, () => api.powerplanDelete(p.guid));
                      }}
                      disabled={busy !== null || p.active || !admin}
                    >
                      🗑
                    </button>
                  )}
                </div>
              </div>
            ))}

            {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 10 }}>{log}</div>}

            <div style={{ marginTop: 14 }}>
              {!showCreate ? (
                <button className="btn small ghost" onClick={() => setShowCreate(true)} disabled={!admin}>
                  + Create custom plan…
                </button>
              ) : (
                <div className="row" style={{ gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                  <input
                    placeholder="Plan name"
                    value={newName}
                    onChange={e => setNewName(e.target.value)}
                    style={{ padding: "4px 8px", fontSize: 13, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)", width: 200 }}
                  />
                  <span className="muted" style={{ fontSize: 12 }}>based on</span>
                  <button
                    className="btn small ghost"
                    onClick={() => act("create", () => api.powerplanCreate(newName, activeGuid))}
                    disabled={!newName.trim() || busy !== null}
                  >
                    Duplicate active plan
                  </button>
                  <button className="btn small ghost" onClick={() => setShowCreate(false)}>Cancel</button>
                </div>
              )}
            </div>
          </>
        )}
      </Card>

      <div className="muted" style={{ fontSize: 12, marginTop: 14 }}>
        Tip: Ultimate Performance keeps CPU at max clock — increases idle power draw by 10–30W.
        Switch back to Balanced when not gaming.
      </div>
    </>
  );
}
