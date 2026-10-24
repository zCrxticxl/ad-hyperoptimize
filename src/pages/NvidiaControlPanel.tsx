import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";
import { engineText } from "../lib/engineText";

type OpenEntry = { id: string; name: string; time: string };
type NvStatus = { supported: boolean; name?: string; openEntries?: OpenEntry[] };

/** The global 3D settings this page used to write are ignored by current
 * NVIDIA drivers. It now opens NVIDIA's own panel and keeps Undo for
 * entries applied by earlier versions. */
export default function NvidiaControlPanel({ admin }: { admin: boolean }) {
  const { t, lang } = useLang();
  const [data, setData] = useState<NvStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [log, setLog] = useState("");
  const [opening, setOpening] = useState(false);

  const refresh = () => {
    setLoading(true);
    setErr("");
    api.nvStatus()
      .then(setData)
      .catch((e: any) => setErr(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => { refresh(); }, []);

  const undo = async (id: string) => {
    setBusy(id);
    setLog("");
    try {
      await api.revertEntry(id);
      setLog(t("nvcpUndone"));
      refresh();
    } catch (e: any) {
      setLog(String(e));
    } finally {
      setBusy(null);
    }
  };

  const openPanel = async () => {
    setOpening(true);
    try { await api.nvOpenPanel(); }
    catch (e: any) { setLog(engineText(String(e), lang)); }
    finally { setOpening(false); }
  };

  const open = data?.openEntries ?? [];

  return (
    <>
      <h1 className="page-title">🟢 {t("nvcpTitle")}</h1>
      <div className="page-sub">{t("nvcpSub")}</div>

      {loading && <><Spinner /> <span className="muted">{t("gpuLoading")}</span></>}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}

      {data && !data.supported && (
        <div style={{ color: "var(--orange)", fontSize: 13, marginBottom: 12, padding: "10px 14px", background: "rgba(255,160,0,0.08)", borderRadius: 6, border: "1px solid var(--orange)" }}>
          ⚠ {t("nvcpNoGpu")}
        </div>
      )}

      <Card title={t("nvcpMoreSettings")}>
        <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>{t("nvcpMoreSettingsDesc")}</div>
        <button className="btn small" disabled={opening} onClick={openPanel}>
          {opening ? <Spinner /> : `↗ ${t("nvcpOpenPanel")}`}
        </button>
      </Card>

      {open.length > 0 && (
        <Card title={t("nvcpOpenEntries")} style={{ marginTop: 14 }}>
          <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>{t("nvcpRetiredNote")}</div>
          {open.map((e) => (
            <div key={e.id} className="row" style={{ padding: "8px 0", borderBottom: "1px solid var(--border)", gap: 12, alignItems: "center" }}>
              <div style={{ flex: 1, fontSize: 13 }}>
                {e.name}
                <div className="muted" style={{ fontSize: 11 }}>{new Date(e.time).toLocaleString()}</div>
              </div>
              <button className="btn small ghost" disabled={busy !== null || !admin} onClick={() => undo(e.id)}>
                {busy === e.id ? <Spinner /> : t("revert")}
              </button>
            </div>
          ))}
          {!admin && <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>{t("adminRestart")}</div>}
        </Card>
      )}

      {log && <div className="mono muted" style={{ fontSize: 12, marginTop: 12 }}>{log}</div>}
    </>
  );
}
