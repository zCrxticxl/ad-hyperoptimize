import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, ActionBtn, RawJson } from "../components/ui";

export default function Reports() {
  const [report, setReport] = useState<any | null>(null);
  const [history, setHistory] = useState<any[]>([]);
  const [restorePts, setRestorePts] = useState<any | null>(null);
  const [logs, setLogs] = useState<any | null>(null);
  const [health, setHealth] = useState<any | null>(null);

  useEffect(() => {
    api.history().then(setHistory).catch(() => {});
  }, []);

  return (
    <>
      <div className="page-title">Reports & History</div>
      <div className="page-sub">Full HTML/JSON system reports, change history, restore points and event-log correlation.</div>

      <Card title="Generate system report">
        <div className="row">
          <ActionBtn
            label="Generate HTML + JSON report"
            onRun={async () => setReport(await api.generateReport())}
          />
          {report && (
            <>
              <button className="btn ghost" onClick={() => api.openPath(report.htmlPath)}>Open HTML report</button>
              <button className="btn ghost" onClick={() => api.openPath(report.jsonPath)}>Open JSON</button>
            </>
          )}
        </div>
        {report && <div className="muted mt mono">{report.htmlPath}</div>}
        <div className="muted mt">Use the browser's print dialog on the HTML report for a PDF copy.</div>
      </Card>

      <Card title={`Optimization change history (${history.length})`} style={{ marginTop: 14 }}>
        <table className="tbl">
          <thead><tr><th>Time</th><th>Tweak</th><th>State</th><th>Backups</th></tr></thead>
          <tbody>
            {[...history].reverse().map((h, i) => (
              <tr key={i}>
                <td className="muted">{new Date(h.time).toLocaleString()}</td>
                <td>{h.tweak_name}</td>
                <td style={{ color: h.reverted ? "var(--muted)" : "var(--green)" }}>{h.reverted ? "reverted" : "applied"}</td>
                <td className="muted">{h.backup_files?.length ?? 0} .reg file(s)</td>
              </tr>
            ))}
            {history.length === 0 && <tr><td colSpan={4} className="muted">No changes made yet.</td></tr>}
          </tbody>
        </table>
      </Card>

      <div className="grid grid-2 mt">
        <Card title="System restore points">
          {!restorePts ? (
            <button className="btn ghost small" onClick={() => api.listRestorePoints().then(setRestorePts)}>Load restore points</button>
          ) : (
            <RawJson label="Restore points" data={restorePts} />
          )}
        </Card>
        <Card title="Windows component health (DISM)">
          {!health ? (
            <button className="btn ghost small" onClick={() => api.componentHealth().then(setHealth)}>Check component store</button>
          ) : (
            <RawJson label="DISM CheckHealth output" data={health} />
          )}
        </Card>
      </div>

      <Card title="Event log correlation (7 days: errors, BSOD, minidumps)" style={{ marginTop: 14 }}>
        {!logs ? (
          <button className="btn ghost small" onClick={() => api.eventLogs().then(setLogs)}>Analyze event logs</button>
        ) : (
          <RawJson label="Critical events / BSOD / minidumps" data={logs} />
        )}
      </Card>
    </>
  );
}
