import React, { useState } from "react";
import { api } from "../api";
import { Card, ActionBtn, RawJson } from "../components/ui";
import { useLang } from "../i18n";
import { RecoveryTimeline } from "../components/RecoveryTimeline";

export default function Reports() {
  const { t } = useLang();
  const [report, setReport] = useState<any | null>(null);
  const [restorePts, setRestorePts] = useState<any | null>(null);
  const [logs, setLogs] = useState<any | null>(null);
  const [health, setHealth] = useState<any | null>(null);

  return (
    <>
      <h1 className="page-title">{t("repTitle")}</h1>
      <div className="page-sub">{t("repSub")}</div>
      <RecoveryTimeline />

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


      <div className="grid grid-2 mt">
        <Card title={t("repSystemRestorePoints")}>
          {!restorePts ? (
            <button className="btn ghost small" onClick={() => api.listRestorePoints().then(setRestorePts).catch((e: any) => setRestorePts({ error: String(e) }))}>{t("repLoadRestorePoints")}</button>
          ) : (
            <RawJson label={t("repRestorePoints")} data={restorePts} />
          )}
        </Card>
        <Card title={t("repComponentHealth")}>
          {!health ? (
            <button className="btn ghost small" onClick={() => api.componentHealth().then(setHealth).catch((e: any) => setHealth({ error: String(e) }))}>{t("repCheckComponentStore")}</button>
          ) : (
            <RawJson label={t("repDismOutput")} data={health} />
          )}
        </Card>
      </div>

      <Card title={t("repEventLogCorrelation")} style={{ marginTop: 14 }}>
        {!logs ? (
          <button className="btn ghost small" onClick={() => api.eventLogs().then(setLogs).catch((e: any) => setLogs({ error: String(e) }))}>{t("repAnalyzeEventLogs")}</button>
        ) : (
          <RawJson label={t("repCriticalEvents")} data={logs} />
        )}
      </Card>
    </>
  );
}
