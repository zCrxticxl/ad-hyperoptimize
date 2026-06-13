import React, { useEffect, useState } from "react";
import { api, fmtAge } from "../api";
import { Card, Stat, Badge, Spinner, RawJson } from "../components/ui";
import type { Mode } from "../App";

export default function Dashboard({ mode, go }: { mode: Mode; go: (p: string) => void }) {
  const [analysis, setAnalysis] = useState<any | null>(null);
  const [meta, setMeta] = useState<{ time: string; fromCache: boolean } | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);

  const run = async (force: boolean) => {
    setBusy(true);
    setErr("");
    try {
      const env = await api.analyze(force);
      setAnalysis(env.data);
      setMeta({ time: env.time, fromCache: env.fromCache });
    } catch (e: any) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    run(false); // instant from cache after the first ever run
  }, []);

  const score = analysis?.healthScore ?? null;
  const color = score === null ? undefined : score >= 80 ? "var(--green)" : score >= 50 ? "var(--yellow)" : "var(--red)";

  return (
    <>
      <div className="page-title">Dashboard</div>
      <div className="page-sub">Analysis-engine summary of your system's health, performance and security posture.</div>

      <div className="grid grid-3">
        <Card title="Health score">
          {busy && !analysis ? (
            <Spinner />
          ) : (
            <>
              <div className="health-ring" style={{ color }}>
                {score ?? "—"}<span style={{ fontSize: 18, color: "var(--muted)" }}>/100</span>
              </div>
              <button className="btn small ghost mt" onClick={() => run(true)} disabled={busy}>
                {busy ? "Analyzing…" : "Re-analyze"}
              </button>
              {meta && (
                <div className="stat-sub">
                  {meta.fromCache ? "cached · " : ""}analyzed {fmtAge(meta.time)}
                </div>
              )}
            </>
          )}
        </Card>
        <Card title="Summary" style={{ gridColumn: "span 2" }}>
          <div style={{ lineHeight: 1.6 }}>{analysis?.summary ?? (busy ? "Running full analysis…" : "—")}</div>
          {err && <div style={{ color: "var(--red)" }}>{err}</div>}
        </Card>
      </div>

      <div className="mt" />
      <Card title={`Findings ${analysis ? `(${analysis.findings.length})` : ""}`}>
        {!analysis && busy && <Spinner />}
        {analysis?.findings.length === 0 && <div className="muted">No issues found.</div>}
        {analysis?.findings.map((f: any, i: number) => (
          <div key={i} className="tweak">
            <div className="tweak-head">
              <Badge cls={`sev-${f.severity}`}>
                {["", "INFO", "LOW", "MEDIUM", "HIGH", "CRITICAL"][f.severity]}
              </Badge>
              <span className="tweak-name">{f.title}</span>
              {f.tweakIds.length > 0 && (
                <button className="btn small ghost" onClick={() => go("optimize")}>
                  Fix in Optimize →
                </button>
              )}
            </div>
            <div className="tweak-desc">{f.detail}</div>
            <div className="tweak-desc" style={{ color: "var(--cyan)" }}>→ {f.recommendation}</div>
          </div>
        ))}
        {mode === "expert" && analysis && <RawJson data={analysis} />}
      </Card>
    </>
  );
}
