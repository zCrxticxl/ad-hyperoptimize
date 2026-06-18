import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Badge, Spinner, ActionBtn } from "../components/ui";
import { RiskBadge, RiskNotice } from "../components/HwWarnings";
import { useHwProfile } from "../hooks/useHwProfile";
import { useLang } from "../i18n";
import { localizeTweak } from "../localize";
import type { Mode } from "../App";

export default function Optimize({ mode, admin }: { mode: Mode; admin: boolean }) {
  const [tweaks, setTweaks] = useState<any[] | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [rpStatus, setRpStatus] = useState("");
  const [confirm, setConfirm] = useState<string | null>(null);
  const [riskAck, setRiskAck] = useState<string | null>(null);
  const profile = useHwProfile();
  const { lang } = useLang();

  const refresh = () => api.listTweaks().then(setTweaks);
  useEffect(() => { refresh(); }, []);

  const push = (m: string) => setLog((l) => [...l.slice(-200), `[${new Date().toLocaleTimeString()}] ${m}`]);

  const visible = (tweaks ?? []).filter((t) => mode === "expert" || t.risk === "Low");
  const cats = [...new Set(visible.map((t) => t.category))];

  const doApply = async (t: any) => {
    push(`Applying: ${t.name}…`);
    try {
      await api.applyTweak(t.id);
      push(`✔ Applied ${t.name} (backup + journal written — undo available)`);
    } catch (e: any) {
      push(`✘ ${t.name}: ${e}`);
    }
    setConfirm(null);
    setRiskAck(null);
    refresh();
  };

  const doRevert = async (t: any) => {
    push(`Reverting: ${t.name}…`);
    try {
      await api.revertTweak(t.id);
      push(`✔ Reverted ${t.name} to previous values`);
    } catch (e: any) {
      push(`✘ ${t.name}: ${e}`);
    }
    refresh();
  };

  return (
    <>
      <div className="page-title">Safe Optimization Engine</div>
      <div className="page-sub">
        Nothing changes without your explicit confirmation. Every tweak is backed up, journaled and individually undoable.
        {mode === "beginner" && " Beginner mode shows Low-risk tweaks only — switch to Expert for the full catalog."}
      </div>

      <Card title="Safety first">
        <div className="row">
          <ActionBtn
            label="Create System Restore Point"
            onRun={async () => {
              try {
                setRpStatus(await api.createRestorePoint("AD HyperOptimize — before optimization"));
              } catch (e: any) {
                setRpStatus(String(e));
              }
            }}
          />
          <span className="muted">{rpStatus || "Recommended before applying Medium-risk tweaks. Requires admin."}</span>
        </div>
      </Card>

      {!tweaks && <div className="mt"><Spinner /> <span className="muted">Reading current system state…</span></div>}

      {cats.map((cat) => (
        <div key={cat} className="mt">
          <h3 style={{ color: "var(--muted)", textTransform: "uppercase", fontSize: 12, letterSpacing: ".5px", marginBottom: 8 }}>{cat}</h3>
          {visible.filter((t) => t.category === cat).map((t) => {
            t = localizeTweak(t, lang);
            const hwRisk = profile?.tweakRisks?.[t.id];
            const needsAck = hwRisk?.severity === "danger" && riskAck !== t.id;
            return (
            <div className="tweak" key={t.id}>
              <div className="tweak-head">
                <span className="tweak-name">{t.name}</span>
                <Badge cls={`risk-${t.risk}`}>{t.risk} risk</Badge>
                <RiskBadge id={t.id} />
                <Badge cls={`st-${t.status}`}>{t.status.replace("_", " ")}</Badge>
                {t.requiresAdmin && <Badge cls="st-unknown">admin</Badge>}
                <button className="btn small ghost" onClick={() => setOpen(open === t.id ? null : t.id)}>
                  {open === t.id ? "Hide" : "Details"}
                </button>
                {t.status !== "applied" && (
                  confirm === t.id ? (
                    needsAck ? (
                      <button className="btn small ghost" onClick={() => { setConfirm(null); setRiskAck(null); }}>Cancel</button>
                    ) : (
                      <>
                        <button className="btn small danger" onClick={() => doApply(t)}>Confirm apply</button>
                        <button className="btn small ghost" onClick={() => { setConfirm(null); setRiskAck(null); }}>Cancel</button>
                      </>
                    )
                  ) : (
                    <button
                      className="btn small"
                      disabled={t.requiresAdmin && !admin}
                      onClick={() => { setOpen(t.id); setConfirm(t.id); }}
                    >
                      Apply…
                    </button>
                  )
                )}
                {t.undoable && (
                  <button className="btn small ghost" onClick={() => doRevert(t)}>Undo</button>
                )}
              </div>
              <div className="tweak-desc">{t.description}</div>
              {open === t.id && (
                <div className="tweak-detail">
                  <b>Why it matters:</b> {t.rationale}<br />
                  <b>Expected impact:</b> {t.impact}<br />
                  <b>Risk:</b> {t.risk} · <b>Reversible:</b> {t.reversible ? "Yes — one-click undo, registry backup saved first" : "No"}<br />
                  <RiskNotice id={t.id} />
                  {confirm === t.id && needsAck && (
                    <div style={{ marginTop: 8 }}>
                      <button className="btn small danger" onClick={() => setRiskAck(t.id)}>
                        I understand the risk for my hardware — continue
                      </button>
                    </div>
                  )}
                  {confirm === t.id && !needsAck && (
                    <span style={{ color: "var(--yellow)" }}>
                      Review the above, then press <b>Confirm apply</b>. A .reg backup and journal entry are written before any change.
                    </span>
                  )}
                </div>
              )}
            </div>
            );
          })}
        </div>
      ))}

      <Card title="Live log" style={{ marginTop: 14 }}>
        <div className="log-console">
          {log.length === 0 ? <span className="muted">No actions yet.</span> : log.map((l, i) => <div key={i}>{l}</div>)}
        </div>
      </Card>
    </>
  );
}
