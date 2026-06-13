import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, ActionBtn } from "../components/ui";

const KINDS = [
  { id: "cpu", name: "CPU (SHA-256 throughput)", desc: "Single-thread + all-core hashing. Higher = faster." },
  { id: "memory", name: "Memory bandwidth", desc: "256 MB block copy. Higher GB/s = faster." },
  { id: "disk", name: "Disk sequential I/O", desc: "256 MB write+read on the temp drive with fsync." },
];

export default function Benchmark() {
  const [results, setResults] = useState<Record<string, any>>({});
  const [history, setHistory] = useState<any[]>([]);

  const loadHistory = () => api.benchHistory().then(setHistory);
  useEffect(() => { loadHistory(); }, []);

  const fmt = (r: any) => {
    if (!r) return null;
    switch (r.kind) {
      case "cpu":
        return <>Single: <b>{r.singleMBs} MB/s</b> · Multi ({r.threads}t): <b>{r.multiMBs} MB/s</b> · Scaling ×{r.scaling}</>;
      case "memory":
        return <>Copy bandwidth: <b>{r.copyGBs} GB/s</b></>;
      case "disk":
        return <>Write: <b>{r.seqWriteMBs} MB/s</b> · Read: <b>{r.seqReadMBs} MB/s</b> <span className="muted">({r.note})</span></>;
    }
  };

  return (
    <>
      <div className="page-title">Benchmarks</div>
      <div className="page-sub">
        Run before and after optimizations to measure real impact instead of guessing. Results are stored for comparison.
      </div>

      {KINDS.map((k) => (
        <Card title={k.name} key={k.id} style={{ marginBottom: 14 }}>
          <div className="muted" style={{ marginBottom: 10 }}>{k.desc}</div>
          <div className="row">
            <ActionBtn
              label="Run"
              onRun={async () => {
                const r = await api.runBenchmark(k.id);
                setResults((x) => ({ ...x, [k.id]: r }));
                loadHistory();
              }}
            />
            <span>{fmt(results[k.id]) ?? <span className="muted">not run this session</span>}</span>
          </div>
        </Card>
      ))}

      <Card title={`History (${history.length} runs) — before/after comparison`}>
        <table className="tbl">
          <thead><tr><th>Time</th><th>Test</th><th>Result</th></tr></thead>
          <tbody>
            {[...history].reverse().slice(0, 25).map((h, i) => (
              <tr key={i}>
                <td className="muted">{h.time ? new Date(h.time).toLocaleString() : "—"}</td>
                <td>{h.kind}</td>
                <td>{fmt(h)}</td>
              </tr>
            ))}
            {history.length === 0 && <tr><td colSpan={3} className="muted">No runs recorded yet.</td></tr>}
          </tbody>
        </table>
      </Card>
    </>
  );
}
