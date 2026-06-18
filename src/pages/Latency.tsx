import React, { useState } from "react";
import { api } from "../api";
import { Card, ActionBtn, Bar } from "../components/ui";
import { HwWarnings } from "../components/HwWarnings";
import { useLang } from "../i18n";

const RATING_COLOR: Record<string, string> = {
  excellent: "var(--green)",
  good: "var(--cyan)",
  fair: "var(--yellow)",
  poor: "var(--red)",
};

export default function Latency() {
  const { t } = useLang();
  const [probe, setProbe] = useState<any | null>(null);
  const [counters, setCounters] = useState<any[] | null>(null);
  const [trace, setTrace] = useState<string>("");
  const [traceResult, setTraceResult] = useState<any | null>(null);
  const [recording, setRecording] = useState(false);

  return (
    <>
      <div className="page-title">{t("latTitle")}</div>
      <div className="page-sub">
        {t("latSub")}
      </div>
      <HwWarnings page="latency" />

      <Card title={t("latStep1Title")}>
        <div className="muted" style={{ marginBottom: 10 }}>
          {t("latStep1Desc")}
        </div>
        <ActionBtn label={t("latRunProbe")} onRun={async () => setProbe(await api.stallProbe(5))} />
        {probe && !probe.error && (
          <div className="mt">
            <span className="stat-big" style={{ color: RATING_COLOR[probe.rating] }}>
              {probe.rating.toUpperCase()}
            </span>
            <table className="tbl mt">
              <tbody>
                <tr><td className="muted">{t("latMedianStall")}</td><td>{probe.p50us} µs</td></tr>
                <tr><td className="muted">{t("lat99thPercentile")}</td><td>{probe.p99us} µs</td></tr>
                <tr><td className="muted">{t("lat999thPercentile")}</td><td>{probe.p999us} µs</td></tr>
                <tr><td className="muted">{t("latWorstStall")}</td><td style={{ color: RATING_COLOR[probe.rating], fontWeight: 700 }}>{probe.maxUs} µs</td></tr>
                <tr><td className="muted">{t("latStallsOver")}</td><td>{probe.stallsOver250us} / {probe.stallsOver1ms}</td></tr>
              </tbody>
            </table>
            <div className="tweak-detail mt">{probe.explanation}</div>
          </div>
        )}
        {probe?.error && <div className="mt" style={{ color: "var(--red)" }}>{probe.error}</div>}
      </Card>

      <Card title={t("latStep2Title")} style={{ marginTop: 14 }}>
        <div className="muted" style={{ marginBottom: 10 }}>
          {t("latStep2Desc")}
        </div>
        <ActionBtn label={t("latSampleCounters")} onRun={async () => {
          const r = await api.latencyCounters(5);
          setCounters(Array.isArray(r) ? r : [r]);
        }} />
        {counters && !counters[0]?.error && (
          <table className="tbl mt">
            <thead><tr><th>{t("latCore")}</th><th>{t("latAvgDpcPct")}</th><th>{t("latPeakDpcPct")}</th><th>{t("latAvgIntPct")}</th><th>{t("latDpcRate")}</th><th></th></tr></thead>
            <tbody>
              {[...counters]
                .sort((a, b) => String(a.core).localeCompare(String(b.core), undefined, { numeric: true }))
                .map((c: any, i: number) => (
                  <tr key={i}>
                    <td>{c.core}</td>
                    <td style={{ color: c.avgDpcPct > 2 ? "var(--orange)" : undefined }}>{c.avgDpcPct}</td>
                    <td style={{ color: c.maxDpcPct > 10 ? "var(--red)" : undefined }}>{c.maxDpcPct}</td>
                    <td>{c.avgIntPct}</td>
                    <td>{c.avgDpcRate}</td>
                    <td style={{ width: 120 }}><Bar pct={Math.min(100, c.avgDpcPct * 10)} color={c.avgDpcPct > 2 ? "var(--orange)" : undefined} /></td>
                  </tr>
                ))}
            </tbody>
          </table>
        )}
        {counters && counters[0]?.error && <div className="mt" style={{ color: "var(--red)" }}>{counters[0].error}</div>}
      </Card>

      <Card title={t("latStep3Title")} style={{ marginTop: 14 }}>
        <div className="muted" style={{ marginBottom: 10 }}>
          {t("latStep3Desc")}
        </div>
        <div className="row">
          {!recording ? (
            <ActionBtn label={t("latStartRecording")} onRun={async () => {
              try { setTrace(await api.wprStart()); setRecording(true); setTraceResult(null); }
              catch (e: any) { setTrace(String(e)); }
            }} />
          ) : (
            <>
              <ActionBtn label={t("latStopSave")} className="btn danger" onRun={async () => {
                try { setTraceResult(await api.wprStop()); setTrace(""); setRecording(false); }
                catch (e: any) { setTrace(String(e)); }
              }} />
              <ActionBtn label={t("cancel")} className="btn ghost" onRun={async () => {
                setTrace(await api.wprCancel()); setRecording(false);
              }} />
            </>
          )}
        </div>
        {trace && <div className="mt" style={{ color: recording ? "var(--yellow)" : "var(--muted)" }}>{trace}</div>}
        {traceResult && (
          <div className="mt">
            <div style={{ color: "var(--green)" }}>✔ {t("latTraceSaved")}</div>
            <div className="mono muted">{traceResult.etlPath}</div>
            <div className="tweak-detail mt">{traceResult.next}</div>
            <button className="btn ghost small mt" onClick={() => api.openPath(traceResult.etlPath.replace(/\\[^\\]+$/, ""))}>
              {t("latOpenTraceFolder")}
            </button>
          </div>
        )}
      </Card>
    </>
  );
}
