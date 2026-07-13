import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, ActionBtn, RawJson } from "../components/ui";
import { useLang } from "../i18n";

export default function Reports() {
  const { t } = useLang();
  const [report, setReport] = useState<any | null>(null);
  const [history, setHistory] = useState<any[]>([]);
  const [restorePts, setRestorePts] = useState<any | null>(null);
  const [logs, setLogs] = useState<any | null>(null);
  const [health, setHealth] = useState<any | null>(null);
  const [undoBusy, setUndoBusy] = useState<string | null>(null);
  const [undoErr, setUndoErr] = useState<string>("");

  const loadHistory = () => api.history().then(setHistory).catch(() => {});

  useEffect(() => {
    loadHistory();
  }, []);

  const undoEntry = async (id: string) => {
    setUndoBusy(id);
    setUndoErr("");
    try {
      await api.revertEntry(id);
      await loadHistory();
    } catch (e: any) { setUndoErr(String(e)); }
    finally { setUndoBusy(null); }
  };

  return (
    <>
      <div className="page-title">{t("repTitle")}</div>
      <div className="page-sub">{t("repSub")}</div>

      <Card title={t("repGenerateReport")}>
        <div className="row">
          <ActionBtn
            label={t("repGenerateHtmlJson")}
            onRun={async () => setReport(await api.generateReport())}
          />
          {report && (
            <>
              <button className="btn ghost" onClick={() => api.openPath(report.htmlPath)}>{t("repOpenHtml")}</button>
              <button className="btn ghost" onClick={() => api.openPath(report.jsonPath)}>{t("repOpenJson")}</button>
            </>
          )}
        </div>
        {report && <div className="muted mt mono">{report.htmlPath}</div>}
        <div className="muted mt">{t("repPrintHint")}</div>
      </Card>

      <Card title={`${t("repChangeHistory")} (${history.length})`} style={{ marginTop: 14 }}>
        <table className="tbl">
          <thead><tr><th>{t("repTime")}</th><th>{t("repTweak")}</th><th>{t("repState")}</th><th>{t("repBackups")}</th><th></th></tr></thead>
          <tbody>
            {[...history].reverse().map((h, i) => (
              <tr key={h.id ?? i}>
                <td className="muted">{new Date(h.time).toLocaleString()}</td>
                <td>{h.tweak_name}</td>
                <td style={{ color: h.reverted ? "var(--muted)" : "var(--green)" }}>{h.reverted ? t("repReverted") : t("repApplied")}</td>
                <td className="muted">{h.backup_files?.length ?? 0} {t("repRegFiles")}</td>
                <td>
                  {!h.reverted && h.id && (
                    <button className="btn small ghost" disabled={undoBusy === h.id} onClick={() => undoEntry(h.id)}>
                      {undoBusy === h.id ? "…" : t("revert")}
                    </button>
                  )}
                </td>
              </tr>
            ))}
            {history.length === 0 && <tr><td colSpan={5} className="muted">{t("repNoChanges")}</td></tr>}
          </tbody>
        </table>
        {undoErr && <div className="muted" style={{ fontSize: 12, marginTop: 8, color: "var(--orange)" }}>{undoErr}</div>}
      </Card>

      <div className="grid grid-2 mt">
        <Card title={t("repSystemRestorePoints")}>
          {!restorePts ? (
            <button className="btn ghost small" onClick={() => api.listRestorePoints().then(setRestorePts)}>{t("repLoadRestorePoints")}</button>
          ) : (
            <RawJson label={t("repRestorePoints")} data={restorePts} />
          )}
        </Card>
        <Card title={t("repComponentHealth")}>
          {!health ? (
            <button className="btn ghost small" onClick={() => api.componentHealth().then(setHealth)}>{t("repCheckComponentStore")}</button>
          ) : (
            <RawJson label={t("repDismOutput")} data={health} />
          )}
        </Card>
      </div>

      <Card title={t("repEventLogCorrelation")} style={{ marginTop: 14 }}>
        {!logs ? (
          <button className="btn ghost small" onClick={() => api.eventLogs().then(setLogs)}>{t("repAnalyzeEventLogs")}</button>
        ) : (
          <RawJson label={t("repCriticalEvents")} data={logs} />
        )}
      </Card>
    </>
  );
}
