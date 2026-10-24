import React, { useEffect, useState } from "react";
import { api } from "../../api";
import { Card, Spinner } from "../ui";
import { useLang, TKey } from "../../i18n";

type F = { id: string; kind: string; severity: "security" | "harmful" | "notDefault"; target: string; current: string; default: string; ownedByApp: boolean; resetEntry: string | null };
const SEV = { security: "var(--red)", harmful: "var(--orange)", notDefault: "var(--muted)" } as const;
const ORDER = { security: 0, harmful: 1, notDefault: 2 } as const;

/** Leftovers of other optimizers, with a journaled reset per finding. */
export function LeftoversCard({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData] = useState<{ findings: F[]; tools: string[]; resets: { id: string; entryId: string; name: string }[]; error?: string } | null>(null);
  const [busy, setBusy] = useState("");
  const [msg, setMsg] = useState("");

  const load = () => { setData(null); api.leftoversScan().then(setData).catch((e) => setData({ findings: [], tools: [], resets: [], error: String(e) })); };
  useEffect(load, []);

  const reset = async (f: F) => {
    if (!window.confirm(t("loConfirm").replace("{t}", f.target))) return;
    setBusy(f.id); setMsg("");
    try { await api.leftoverReset(f.id); setMsg(t("loDone")); } catch (e: any) { setMsg(String(e)); }
    finally { setBusy(""); load(); }
  };
  const undo = async (entry: string) => {
    setBusy(entry); setMsg("");
    try { await api.revertEntry(entry); setMsg(t("hwUndone")); } catch (e: any) { setMsg(String(e)); }
    finally { setBusy(""); load(); }
  };

  const findings = [...(data?.findings ?? [])].sort((a, b) => ORDER[a.severity] - ORDER[b.severity]);

  return (
    <Card title={t("loTitle")}>
      <div className="row" style={{ justifyContent: "space-between", gap: 8 }}>
        <div className="muted" style={{ fontSize: 12 }}>{t("loDesc")}</div>
        <button className="btn small ghost" onClick={load}>↺</button>
      </div>
      {!data && <Spinner />}
      {data?.error && <div style={{ color: "var(--red)", fontSize: 12 }}>{data.error}</div>}
      {data && !data.error && findings.length === 0 && <div style={{ color: "var(--green)", fontSize: 13, marginTop: 6 }}>✓ {t("loClean")}</div>}
      {findings.map((f) => (
        <div key={f.id} style={{ padding: "7px 0", borderBottom: "1px solid var(--border)" }}>
          <div className="row" style={{ gap: 8, alignItems: "center", flexWrap: "wrap" }}>
            <span style={{ fontSize: 11, color: SEV[f.severity], fontWeight: 700, minWidth: 90 }}>{t(`loSev_${f.severity}` as TKey)}</span>
            <span style={{ fontSize: 13, flex: 1 }}>{t(`lo_${f.id.startsWith("svc_") ? "service" : f.id}` as TKey).replace("{s}", f.target)}</span>
            {f.ownedByApp ? (
              <span className="muted" style={{ fontSize: 12 }}>{t("loOwned")}</span>
            ) : f.resetEntry ? (
              <button className="btn small ghost" disabled={!!busy || !admin} onClick={() => undo(f.resetEntry!)}>{t("revert")}</button>
            ) : (
              <button className="btn small" disabled={!!busy || !admin} onClick={() => reset(f)}>{busy === f.id ? <Spinner /> : t("hwReset")}</button>
            )}
          </div>
          <div className="muted mono" style={{ fontSize: 11, marginTop: 2 }}>{f.target}: {f.current} → {f.default}</div>
        </div>
      ))}
      {data?.resets?.map((r) => (
        <div key={r.entryId} className="row" style={{ gap: 8, alignItems: "center", padding: "6px 0", borderBottom: "1px solid var(--border)" }}>
          <span style={{ fontSize: 13, flex: 1, color: "var(--green)" }}>✓ {r.name}</span>
          <button className="btn small ghost" disabled={!!busy || !admin} onClick={() => undo(r.entryId)}>{t("revert")}</button>
        </div>
      ))}
      {data && data.tools.length > 0 && (
        <div style={{ fontSize: 12, marginTop: 8, color: "var(--orange)" }}>● {t("loTools").replace("{t}", data.tools.join(", "))}</div>
      )}
      {findings.length > 0 && <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>{t("loRebootNote")}</div>}
      {!admin && findings.length > 0 && <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>{t("adminRestart")}</div>}
      {msg && <div className="muted" style={{ fontSize: 12, marginTop: 6 }}>{msg}</div>}
    </Card>
  );
}
