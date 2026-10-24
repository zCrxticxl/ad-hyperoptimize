import React, { useEffect, useState } from "react";
import { api } from "../../api";
import { Card, Spinner } from "../ui";
import { useLang, TKey } from "../../i18n";

type Session = { id: string; started: string; seconds: number };
type Finding = {
  condition: string; stutterSeconds: number; sessionSeconds: number;
  rateDuringStutter: number; rateOverall: number; lift: number; likely: boolean;
};
type Analysis = {
  id: string; error?: string; game?: string; gameGuessed?: boolean; seconds?: number;
  events?: { condition: string; seconds: number }[];
  capture?: { frames: number; seconds: number; stutters: number; stutterSeconds: number; overlapSeconds: number };
  findings?: Finding[];
};

const mmss = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;

/** Records what the PC does each second while you play; with a frame-time
 * capture of the same session it shows what coincides with stutters. */
export function StutterCard() {
  const { t } = useLang();
  const [status, setStatus] = useState<any>(null);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");
  const [result, setResult] = useState<Analysis | null>(null);

  const label = (c: string) => {
    const m = c.match(/^proc:(.+):(cpu|disk)$/);
    if (m) return `${m[1]} · ${m[2] === "cpu" ? t("stuCondCpu") : t("stuCondDisk")}`;
    if (c.startsWith("gpu:")) return `GPU · ${t(`thrReason_${c.slice(4)}` as TKey)}`;
    return t(`stuCond_${c}` as TKey);
  };

  const loadList = () => api.sessionList().then((r) => setSessions(r.sessions ?? [])).catch(() => {});
  useEffect(() => {
    loadList();
    let alive = true;
    const poll = () => api.sessionStatus().then((s) => alive && setStatus(s)).catch(() => {});
    poll();
    const id = window.setInterval(poll, 1000);
    return () => { alive = false; window.clearInterval(id); };
  }, []);

  const start = async () => {
    setBusy(true); setMsg("");
    try { setStatus(await api.sessionStart()); } catch (e: any) { setMsg(String(e)); } finally { setBusy(false); }
  };
  const stop = async () => {
    setBusy(true); setMsg("");
    try {
      const r = await api.sessionStop();
      setMsg(r.saved ? t("stuSaved") : String(r.reason ?? ""));
      await loadList();
      if (r.saved) analyze(r.id);
    } catch (e: any) { setMsg(String(e)); } finally { setBusy(false); }
  };
  const analyze = async (id: string, file?: File) => {
    setBusy(true); setMsg(""); setResult(null);
    try {
      const r = file ? await api.sessionAnalyze(id, await file.text(), file.lastModified) : await api.sessionAnalyze(id);
      setResult(r);
    } catch (e: any) { setMsg(String(e)); } finally { setBusy(false); }
  };
  const del = async (id: string) => {
    if (!window.confirm(t("stuConfirmDelete"))) return;
    try { await api.sessionDelete(id); if (result?.id === id) setResult(null); loadList(); } catch (e: any) { setMsg(String(e)); }
  };

  const running = !!status?.running;
  const last = status?.last;

  return (
    <Card title={t("stuTitle")}>
      <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>{t("stuDesc")}</div>
      <div className="row" style={{ gap: 10, alignItems: "center", flexWrap: "wrap" }}>
        {running ? (
          <button className="btn danger small" disabled={busy} onClick={stop}>■ {t("stuStop")}</button>
        ) : (
          <button className="btn small" disabled={busy} onClick={start}>● {t("stuStart")}</button>
        )}
        {running && <span style={{ color: "var(--red)", fontSize: 13 }}>● REC {mmss(status.seconds ?? 0)}</span>}
        {status?.capReached && <span className="muted" style={{ fontSize: 12 }}>{t("stuCap")}</span>}
        {busy && <Spinner />}
      </div>
      {running && last && (
        <div className="muted mono" style={{ fontSize: 11, marginTop: 8 }}>
          CPU {last.cpu?.toFixed?.(0)}% · DPC {last.dpc ?? "–"}% · {t("stuHardFaults")} {last.hard_faults ?? "–"}/s
          {last.top?.length ? ` · ${last.top.slice(0, 3).map((p: any) => `${p.name} ${p.cpu}%`).join(", ")}` : ""}
        </div>
      )}
      <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>{t("stuHowTo")}</div>
      {msg && <div style={{ fontSize: 12, marginTop: 8 }}>{msg}</div>}

      {sessions.length > 0 && (
        <div style={{ marginTop: 12 }}>
          {sessions.map((s) => (
            <div key={s.id} className="row" style={{ gap: 8, alignItems: "center", padding: "6px 0", borderBottom: "1px solid var(--border)", flexWrap: "wrap" }}>
              <span style={{ flex: 1, fontSize: 13 }}>{new Date(s.started).toLocaleString()} · {mmss(s.seconds)}</span>
              <button className="btn small ghost" disabled={busy} onClick={() => analyze(s.id)}>{t("stuAnalyze")}</button>
              <label className="btn small ghost" style={{ cursor: busy ? "not-allowed" : "pointer" }}>
                {t("stuWithCsv")}
                <input type="file" accept=".csv" hidden disabled={busy}
                  onChange={(e) => { const f = e.target.files?.[0]; e.target.value = ""; if (f) analyze(s.id, f); }} />
              </label>
              <button className="btn small ghost" onClick={() => del(s.id)}>🗑</button>
            </div>
          ))}
        </div>
      )}

      {result && (
        <div style={{ marginTop: 14 }}>
          {result.error ? (
            <div style={{ color: "var(--orange)", fontSize: 13 }}>{result.error}</div>
          ) : (
            <>
              {result.game && (
                <div className="muted" style={{ fontSize: 12, marginBottom: 6 }}>
                  {result.gameGuessed ? t("stuGameGuessed") : t("stuGame")}: <b>{result.game}</b> ({t("stuGameExcluded")})
                </div>
              )}
              {result.capture && (
                <div style={{ fontSize: 13, marginBottom: 8 }}>
                  {t("stuStutters").replace("{n}", String(result.capture.stutters)).replace("{s}", String(result.capture.stutterSeconds)).replace("{o}", String(result.capture.overlapSeconds))}
                </div>
              )}
              {result.findings && (
                result.findings.length === 0 ? <div className="muted" style={{ fontSize: 13 }}>{t("stuNoFindings")}</div> : (
                  <table className="tbl" style={{ fontSize: 12 }}>
                    <thead><tr><th>{t("stuColCondition")}</th><th>{t("stuColDuring")}</th><th>{t("stuColOverall")}</th><th></th></tr></thead>
                    <tbody>
                      {result.findings.map((f) => (
                        <tr key={f.condition}>
                          <td>{label(f.condition)}</td>
                          <td>{f.rateDuringStutter}% ({f.stutterSeconds} s)</td>
                          <td>{f.rateOverall}% ({f.sessionSeconds} s)</td>
                          <td style={{ color: f.likely ? "var(--orange)" : "var(--muted)" }}>{f.likely ? t("stuLikely") : t("stuCoincidence")}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )
              )}
              {result.events && (
                result.events.length === 0 ? <div className="muted" style={{ fontSize: 13 }}>{t("stuNoEvents")}</div> : (
                  <>
                    <div className="muted" style={{ fontSize: 12, marginBottom: 6 }}>{t("stuEventsHint")}</div>
                    {result.events.slice(0, 15).map((e) => (
                      <div key={e.condition} style={{ fontSize: 13, padding: "3px 0" }}>{label(e.condition)} · <span className="muted">{e.seconds} s</span></div>
                    ))}
                  </>
                )
              )}
              <div className="muted" style={{ fontSize: 11, marginTop: 8 }}>{t("stuDisclaimer")}</div>
            </>
          )}
        </div>
      )}
    </Card>
  );
}
