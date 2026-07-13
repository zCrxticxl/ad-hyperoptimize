import React, { useEffect, useState } from "react";
import { api, fmtAge } from "../api";
import { Card, Stat, Badge, Spinner, RawJson } from "../components/ui";
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

  const SEV_LABELS = ["", t("dashSevInfo"), t("dashSevLow"), t("dashSevMedium"), t("dashSevHigh"), t("dashSevCritical")];

  return (
    <>
      <div className="page-title">{t("dashTitle")}</div>
      <div className="page-sub">{t("dashSub")}</div>

      <div className="grid grid-3">
        <Card title={t("dashHealthScore")}>
          {busy && !analysis ? (
            <Spinner />
          ) : (
            <>
              <div className="health-ring" style={{ color }}>
                {score ?? "—"}<span style={{ fontSize: 18, color: "var(--muted)" }}>/100</span>
              </div>
              <button className="btn small ghost mt" onClick={() => run(true)} disabled={busy}>
                {busy ? t("dashAnalyzing") : t("dashReanalyze")}
              </button>
              {meta && (
                <div className="stat-sub">
                  {meta.fromCache ? `${t("dashCached")} · ` : ""}{t("dashAnalyzed")} {fmtAge(meta.time)}
                </div>
              )}
            </>
          )}
        </Card>
        <Card title={t("dashSummary")} style={{ gridColumn: "span 2" }}>
          <div style={{ lineHeight: 1.6 }}>
            {analysis ? localizeSummary(analysis.findings, t) : (busy ? t("dashRunningFullAnalysis") : "—")}
          </div>
          {err && <div style={{ color: "var(--red)" }}>{err}</div>}
        </Card>
      </div>

      <HwWarnings page="dashboard" />

      <div className="grid grid-2" style={{ marginBottom: 14 }}>
        <Card title={t("dashHardwareProfile")}>
          <HwProfileCard />
        </Card>
      </div>

      <div className="mt" />
      <Card title={`${t("dashFindings")} ${analysis ? `(${analysis.findings.length})` : ""}`}>
        {!analysis && busy && <Spinner />}
        {analysis?.findings.length === 0 && <div className="muted">{t("dashNoIssues")}</div>}
        {analysis?.findings.map((f: any, i: number) => {
          const loc = localizeFinding(f, t);
          return (
          <div key={i} className="tweak">
            <div className="tweak-head">
              <Badge cls={`sev-${f.severity}`}>
                {SEV_LABELS[f.severity]}
              </Badge>
              <span className="tweak-name">{loc.title}</span>
              {f.tweakIds?.length > 0 && (
                <button className="btn small ghost" onClick={() => go("optimize")}>
                  {t("dashFixInOptimize")} →
                </button>
              )}
            </div>
            <div className="tweak-desc">{loc.detail}</div>
            <div className="tweak-desc" style={{ color: "var(--cyan)" }}>→ {loc.recommendation}</div>
          </div>
          );
        })}
        {mode === "expert" && analysis && <RawJson data={analysis} />}
      </Card>
    </>
  );
}
