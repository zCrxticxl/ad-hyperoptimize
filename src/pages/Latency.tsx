import React, { useState } from "react";
import { api } from "../api";
import { Card, ActionBtn, Bar } from "../components/ui";

const RATING_COLOR: Record<string, string> = {
  excellent: "var(--green)",
  good: "var(--cyan)",
  fair: "var(--yellow)",
  poor: "var(--red)",
};

export default function Latency() {
  const [probe, setProbe] = useState<any | null>(null);
  const [counters, setCounters] = useState<any[] | null>(null);
  const [trace, setTrace] = useState<string>("");
  const [traceResult, setTraceResult] = useState<any | null>(null);
  const [recording, setRecording] = useState(false);

  return (
    <>
      <div className="page-title">DPC / Interrupt Latency</div>
      <div className="page-sub">
        Finds the stutter and audio-crackle culprits: long Deferred Procedure Calls and interrupts from misbehaving drivers.
      </div>

      <Card title="1 · Execution stall probe (5 s)">
        <div className="muted" style={{ marginBottom: 10 }}>
          A pinned thread timestamps continuously; any gap is time stolen by DPCs/ISRs/scheduling. Close foreground apps,
          ideally run it again while gaming for a load profile.
        </div>
        <ActionBtn label="Run probe" onRun={async () => setProbe(await api.stallProbe(5))} />
        {probe && !probe.error && (
          <div className="mt">
            <span className="stat-big" style={{ color: RATING_COLOR[probe.rating] }}>
              {probe.rating.toUpperCase()}
            </span>
            <table className="tbl mt">
              <tbody>
                <tr><td className="muted">Median stall</td><td>{probe.p50us} µs</td></tr>
                <tr><td className="muted">99th percentile</td><td>{probe.p99us} µs</td></tr>
                <tr><td className="muted">99.9th percentile</td><td>{probe.p999us} µs</td></tr>
                <tr><td className="muted">Worst stall</td><td style={{ color: RATING_COLOR[probe.rating], fontWeight: 700 }}>{probe.maxUs} µs</td></tr>
                <tr><td className="muted">Stalls &gt; 250 µs / &gt; 1 ms</td><td>{probe.stallsOver250us} / {probe.stallsOver1ms}</td></tr>
              </tbody>
            </table>
            <div className="tweak-detail mt">{probe.explanation}</div>
          </div>
        )}
        {probe?.error && <div className="mt" style={{ color: "var(--red)" }}>{probe.error}</div>}
      </Card>

      <Card title="2 · Per-core DPC / interrupt pressure (≈5 s sample)" style={{ marginTop: 14 }}>
        <div className="muted" style={{ marginBottom: 10 }}>
          Sustained DPC time above ~2% on any core indicates a driver problem; spikes above 10% cause visible hitches.
        </div>
        <ActionBtn label="Sample counters" onRun={async () => {
          const r = await api.latencyCounters(5);
          setCounters(Array.isArray(r) ? r : [r]);
        }} />
        {counters && !counters[0]?.error && (
          <table className="tbl mt">
            <thead><tr><th>Core</th><th>Avg DPC %</th><th>Peak DPC %</th><th>Avg Interrupt %</th><th>DPC rate/s</th><th></th></tr></thead>
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

      <Card title="3 · Deep trace — identify the exact driver (admin)" style={{ marginTop: 14 }}>
        <div className="muted" style={{ marginBottom: 10 }}>
          Records a kernel ETW trace with Windows Performance Recorder (built into Windows). Start, reproduce the
          stutter for 10–30 seconds, stop — then open the .etl in Windows Performance Analyzer and check
          “DPC/ISR Duration by Module”.
        </div>
        <div className="row">
          {!recording ? (
            <ActionBtn label="● Start recording" onRun={async () => {
              try { setTrace(await api.wprStart()); setRecording(true); setTraceResult(null); }
              catch (e: any) { setTrace(String(e)); }
            }} />
          ) : (
            <>
              <ActionBtn label="■ Stop & save .etl" className="btn danger" onRun={async () => {
                try { setTraceResult(await api.wprStop()); setTrace(""); setRecording(false); }
                catch (e: any) { setTrace(String(e)); }
              }} />
              <ActionBtn label="Cancel" className="btn ghost" onRun={async () => {
                setTrace(await api.wprCancel()); setRecording(false);
              }} />
            </>
          )}
        </div>
        {trace && <div className="mt" style={{ color: recording ? "var(--yellow)" : "var(--muted)" }}>{trace}</div>}
        {traceResult && (
          <div className="mt">
            <div style={{ color: "var(--green)" }}>✔ Trace saved</div>
            <div className="mono muted">{traceResult.etlPath}</div>
            <div className="tweak-detail mt">{traceResult.next}</div>
            <button className="btn ghost small mt" onClick={() => api.openPath(traceResult.etlPath.replace(/\\[^\\]+$/, ""))}>
              Open trace folder
            </button>
          </div>
        )}
      </Card>
    </>
  );
}
