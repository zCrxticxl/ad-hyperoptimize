import React, { useEffect, useState } from "react";
import { api, fmtAge } from "../api";
import { Badge, Card, RawJson, Spinner } from "../components/ui";
import { HwWarnings, HwProfileCard } from "../components/HwWarnings";
import { useLang } from "../i18n";
import { localizeFinding, localizeSummary } from "../localize";
import type { Mode } from "../App";

export default function Dashboard({ mode, go }: { mode: Mode; go: (p: string) => void }) {
  const { t } = useLang();
  const [analysis, setAnalysis] = useState<any | null>(null);
  const [meta, setMeta] = useState<{ time: string; fromCache: boolean } | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);

  const run = async (force: boolean) => {
    setBusy(true); setErr("");
    try { const env = await api.analyze(force); setAnalysis(env.data); setMeta({ time: env.time, fromCache: env.fromCache }); }
    catch (error: any) { setErr(String(error)); }
    finally { setBusy(false); }
  };

  useEffect(() => { run(false); }, []);

  const score = analysis?.healthScore ?? null;
  const tone = score === null ? "neutral" : score >= 80 ? "good" : score >= 50 ? "warn" : "bad";
  const findings = analysis?.findings ?? [];
  const critical = findings.filter((finding: any) => finding.severity >= 4).length;
  const actionable = findings.filter((finding: any) => finding.tweakIds?.length > 0).length;
  const severityLabels = ["", t("dashSevInfo"), t("dashSevLow"), t("dashSevMedium"), t("dashSevHigh"), t("dashSevCritical")];

  return <>
    <section className="dashboard-hero">
      <div className="hero-copy">
        <div className="eyebrow">SYSTEM OVERVIEW</div>
        <h1>{t("dashTitle")}</h1>
        <p>{analysis ? localizeSummary(findings, t) : busy ? t("dashRunningFullAnalysis") : t("dashSub")}</p>
        <div className="hero-actions">
          <button className="btn" onClick={() => run(true)} disabled={busy}>{busy ? <><Spinner /> {t("dashAnalyzing")}</> : <>↻ {t("dashReanalyze")}</>}</button>
          {actionable > 0 && <button className="btn ghost" onClick={() => go("optimize")}>Review {actionable} actions →</button>}
        </div>
        {meta && <div className="analysis-meta"><span className="status-dot is-ready" />{meta.fromCache ? `${t("dashCached")} · ` : ""}{t("dashAnalyzed")} {fmtAge(meta.time)}</div>}
        {err && <div className="inline-error">{err}</div>}
      </div>
      <div className={`score-orbit ${tone}`}>
        <div className="score-inner"><span className="score-value">{busy && score === null ? <Spinner /> : score ?? "—"}</span><span className="score-label">{t("dashHealthScore")}</span><span className="score-total">/ 100</span></div>
      </div>
    </section>

    <section className="dashboard-stats">
      <div className="overview-stat"><span className="overview-icon blue">⌁</span><div><b>{findings.length}</b><span>{t("dashFindings")}</span></div></div>
      <div className="overview-stat"><span className={`overview-icon ${critical ? "red" : "green"}`}>{critical ? "!" : "✓"}</span><div><b>{critical}</b><span>{t("dashSevHigh")}</span></div></div>
      <button className="overview-stat action" onClick={() => go("optimize")}><span className="overview-icon violet">↗</span><div><b>{actionable}</b><span>{t("dashFixInOptimize")}</span></div><span className="stat-arrow">→</span></button>
    </section>

    <HwWarnings page="dashboard" />

    <div className="dashboard-grid">
      <Card title={t("dashFindings")} style={{ minHeight: 260 }}>
        {!analysis && busy && <div className="loading-panel"><Spinner /></div>}
        {analysis && findings.length === 0 && <div className="empty-state"><span>✓</span><b>{t("dashNoIssues")}</b><p>{t("dashSub")}</p></div>}
        <div className="finding-list">
          {findings.slice(0, 5).map((finding: any, index: number) => {
            const localized = localizeFinding(finding, t);
            return <div key={index} className="finding-row">
              <Badge cls={`sev-${finding.severity}`}>{severityLabels[finding.severity]}</Badge>
              <div className="finding-copy"><b>{localized.title}</b><span>{localized.recommendation}</span></div>
              {finding.tweakIds?.length > 0 && <button className="row-action" onClick={() => go("optimize")}>Fix →</button>}
            </div>;
          })}
        </div>
        {findings.length > 5 && <button className="text-action" onClick={() => go("healthcheck")}>View all {findings.length} findings →</button>}
      </Card>
      <Card title={t("dashHardwareProfile")}><HwProfileCard /></Card>
    </div>

    {mode === "expert" && analysis && <div className="expert-panel"><RawJson data={analysis} /></div>}
  </>;
}
