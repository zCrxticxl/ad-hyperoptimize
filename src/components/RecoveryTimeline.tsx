import React, { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { TKey, useLang } from "../i18n";
import { Card, Spinner } from "./ui";
import type { RecoverySession, RecoverySetting } from "../lib/diagnosticTypes";

export function RecoveryTimeline() {
  const { t } = useLang();
  const [sessions, setSessions] = useState<RecoverySession[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [result, setResult] = useState<{ session: string; partial: boolean; details: string[] } | null>(null);
  const revision = useRef(0);
  const active = useRef(false);
  const load = async () => {
    const rev = ++revision.current;
    setLoading(true); setError("");
    try { const data = await api.recoveryTimeline(); if (rev === revision.current) setSessions(data.sessions); }
    catch (e) { if (rev === revision.current) setError(String(e)); }
    finally { if (rev === revision.current) setLoading(false); }
  };
  useEffect(() => { void load(); return () => { revision.current++; }; }, []);
  const status = (name: string) => {
    const key = `recovery_${name}` as TKey;
    return ["applied", "incomplete", "partial", "restored", "ready", "alreadyOriginal", "conflict", "newerChange", "unsupported", "untrusted", "unreadable", "notAttempted", "verificationFailed"].includes(name) ? t(key) : name;
  };
  const value = (setting: RecoverySetting | null) => {
    if (!setting) return t("recoveryUnknown");
    if (setting.kind === "Service") return setting.value;
    if (setting.value === null) return t("recoveryAbsent");
    return `${setting.value.t}: ${typeof setting.value.v === "string" ? setting.value.v : JSON.stringify(setting.value.v)}`;
  };
  const undo = async (session: RecoverySession) => {
    if (active.current || !window.confirm(t("recoveryConfirm"))) return;
    active.current = true; setBusy(session.id); setResult(null); setError("");
    let failure = "";
    try {
      const response = await api.recoveryUndo(session.id);
      setResult({ session: session.id, partial: response.results.some(r => !r.ok), details: response.results.filter(r => !r.ok).map(r => `${r.id}: ${status(r.error ?? "unknown")}`) });
    } catch (e) { failure = String(e); }
    finally {
      // A failed request can still have restored and persisted several steps.
      await load();
      if (failure) setError(failure);
      active.current = false; setBusy(null);
    }
  };
  return <Card title={t("recoveryTitle")}>
    <div className="row" style={{ justifyContent: "space-between" }}><p className="muted">{t("recoveryIntro")}</p><button className="btn small ghost" disabled={loading || !!busy} onClick={() => void load()}>{t("diagRefresh")}</button></div>
    {error && <p role="alert" className="diag-error">{error}</p>}
    {loading && <Spinner />}
    {!loading && !error && !sessions.length && <p className="muted">{t("recoveryEmpty")}</p>}
    <div className="recovery-timeline">{sessions.map(session => {
      const open = session.entries.some(e => e.status !== "restored");
      const hasConflict = session.entries.some(e => e.items.some(i => ["conflict", "newerChange", "unreadable", "untrusted", "unsupported"].includes(i.status)));
      return <article className="recovery-session" key={session.id}>
        <div className="row" style={{ justifyContent: "space-between", flexWrap: "wrap" }}>
          <div><h3>{session.label}</h3><div className="muted">{new Date(session.time).toLocaleString()} · {session.entries.length} {t("recoveryEntries")}</div></div>
          {open ? <button className="btn small" disabled={!!busy || loading} onClick={() => void undo(session)}>{busy === session.id ? "…" : t("recoveryUndo")}</button> : <span className="badge">{t("recovery_restored")}</span>}
        </div>
        {session.legacy && <p className="muted">{t("recoveryLegacy")}</p>}
        {hasConflict && open && <p className="diag-warning">{t("recoveryPartialNotice")}</p>}
        {result?.session === session.id && <div role="status" className={result.partial ? "diag-warning" : "diag-success"}>{t(result.partial ? "recoveryPartialNotice" : "recoveryDone")}<ul>{result.details.map(d => <li key={d}>{d}</li>)}</ul></div>}
        {session.entries.map(entry => <details className="recovery-entry" key={entry.id}>
          <summary>{entry.name} <span className="muted"> · {status(entry.status)} · {t("recoveryProgress")}: {entry.restored}/{entry.attempted}</span></summary>
          {entry.items.map((item, i) => <div className="recovery-item" key={i}>
            <code>{item.resource}</code><p className={item.status === "conflict" || item.status === "newerChange" ? "diag-warning" : "muted"}>{status(item.status)}</p>
            <div className="recovery-values">{([["recoveryBefore", item.before], ["recoveryTarget", item.target], ["recoveryCurrent", item.current]] as const).map(([label, setting]) => <div key={label}><b>{t(label)}</b><pre>{value(setting)}</pre></div>)}</div>
            {item.error && <p className="diag-error">{item.error}</p>}
          </div>)}
        </details>)}
      </article>;
    })}</div>
  </Card>;
}
