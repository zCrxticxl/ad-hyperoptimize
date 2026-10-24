import React, { useEffect, useState } from "react";
import { api } from "../../api";
import { Card, Spinner } from "../ui";
import { useLang, TKey } from "../../i18n";

type Act = { activity: string; pausable: boolean; processes: { name: string; cpu: number; diskMbs: number }[] };

/** What competes with a game right now, and a reversible pause for the parts
 * Windows lets an app pause. */
export function PregameCard({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData] = useState<{ activities: Act[]; paused: any } | null>(null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState("");

  const scan = () => {
    setBusy(true);
    api.pregameScan().then(setData).catch((e) => setMsg(String(e))).finally(() => setBusy(false));
  };
  useEffect(scan, []);

  const pausable = (data?.activities ?? []).filter((a) => a.pausable).map((a) => a.activity);
  const pause = async () => {
    setBusy(true); setMsg("");
    try {
      const r = await api.pregamePause(pausable);
      setMsg([t("preDone"), ...(r.notes ?? [])].join(" "));
    } catch (e: any) { setMsg(String(e)); }
    finally { scan(); }
  };
  const resume = async () => {
    setBusy(true); setMsg("");
    try { await api.pregameResume(); setMsg(t("preResumed")); } catch (e: any) { setMsg(String(e)); }
    finally { scan(); }
  };

  const paused = data?.paused && data.paused !== null;

  return (
    <Card title={t("preTitle")}>
      <div className="row" style={{ justifyContent: "space-between", marginBottom: 8, gap: 8 }}>
        <div className="muted" style={{ fontSize: 12 }}>{t("preDesc")}</div>
        <button className="btn small ghost" disabled={busy} onClick={scan}>{busy ? <Spinner /> : "↺"}</button>
      </div>
      {data && data.activities.length === 0 && (
        <div style={{ color: "var(--green)", fontSize: 13 }}>✓ {t("preClean")}</div>
      )}
      {data?.activities.map((a) => (
        <div key={a.activity} style={{ padding: "6px 0", borderBottom: "1px solid var(--border)" }}>
          <div style={{ fontSize: 13 }}>
            <b>{t(`preAct_${a.activity}` as TKey)}</b>{" "}
            <span className="muted" style={{ fontSize: 12 }}>
              {a.processes.map((p) => `${p.name} ${p.cpu}%${p.diskMbs >= 1 ? ` · ${p.diskMbs} MB/s` : ""}`).join(", ")}
            </span>
          </div>
          <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>{t(`preHint_${a.activity}` as TKey)}</div>
        </div>
      ))}
      <div className="row" style={{ gap: 8, marginTop: 10, flexWrap: "wrap" }}>
        {pausable.length > 0 && !paused && (
          <button className="btn small" disabled={busy || !admin} onClick={pause}>⏸ {t("prePause")}</button>
        )}
        {paused && (
          <button className="btn small" disabled={busy || !admin} onClick={resume}>▶ {t("preResume")}</button>
        )}
        {!admin && (pausable.length > 0 || paused) && <span className="muted" style={{ fontSize: 12 }}>{t("adminRestart")}</span>}
      </div>
      {paused && <div style={{ fontSize: 12, marginTop: 6, color: "var(--orange)" }}>{t("prePausedNote")}</div>}
      {msg && <div className="muted" style={{ fontSize: 12, marginTop: 6 }}>{msg}</div>}
    </Card>
  );
}
