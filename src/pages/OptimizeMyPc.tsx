import React, { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Badge, Card, ErrorBanner, Spinner } from "../components/ui";
import { OptCard } from "../components/OptCard";
import { rescanHealth, useHealthScan } from "../hooks/useHealthScan";
import { useLang, TKey } from "../i18n";
import { localizeTweak } from "../localize";
import { ApplyOutcome, ApplyResult } from "../lib/health";
import { engineText } from "../lib/engineText";

const OUTCOME_ORDER: ApplyOutcome[] = ["verified", "needsRestart", "already", "skipped", "unverified", "failed"];
const OUTCOME_CLS: Record<ApplyOutcome, string> = {
  verified: "st-applied",
  needsRestart: "st-partial",
  already: "st-applied",
  skipped: "st-unknown",
  unverified: "st-partial",
  failed: "risk-High",
};

/** One-click optimization: scan → preview → explicit approval → apply →
 * read back every change → honest per-item result. */
export default function OptimizeMyPc({ admin, go }: { admin: boolean; go: (id: string) => void }) {
  const { t, lang } = useLang();
  const { scan, busy, error, rescan } = useHealthScan();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [restorePoint, setRestorePoint] = useState(admin);
  const [applying, setApplying] = useState(false);
  const [result, setResult] = useState<ApplyResult | null>(null);
  const [applyErr, setApplyErr] = useState("");

  const plan = useMemo(() => (scan?.items ?? []).filter((i) => i.oneClick), [scan]);
  const blocked = useMemo(
    () => (scan?.items ?? []).filter((i) => i.diagnosis === "recommended" && i.oneClickCandidate && !i.oneClick),
    [scan],
  );
  const manual = useMemo(
    () => (scan?.items ?? []).filter((i) => i.diagnosis === "recommended" && !i.oneClickCandidate),
    [scan],
  );

  // Preselect the whole plan whenever a new scan arrives.
  useEffect(() => { setSelected(new Set(plan.map((i) => i.id))); }, [plan]);

  const approve = async () => {
    if (applying || selected.size === 0) return;
    setApplying(true);
    setApplyErr("");
    setResult(null);
    try {
      const r: ApplyResult = await api.optimizeApply([...selected], restorePoint && admin);
      if (r.error) setApplyErr(r.error);
      else setResult(r);
    } catch (e) {
      setApplyErr(String(e));
    } finally {
      await rescanHealth().catch(() => {});
      setApplying(false);
    }
  };

  const name = (id: string, fallback: string) => localizeTweak({ id, name: fallback, description: "" }, lang).name;

  return (
    <>
      <h1 className="page-title">⚡ {t("omTitle")}</h1>
      <div className="page-sub">{t("omSub")}</div>
      {error && <ErrorBanner msg={error} onRetry={rescan} />}
      {applyErr && <ErrorBanner msg={applyErr} />}

      {result && (
        <Card title={t("omResults")}>
          <div className="row" style={{ gap: 8, flexWrap: "wrap", marginBottom: 10 }}>
            {OUTCOME_ORDER.filter((o) => result.summary[o] > 0).map((o) => (
              <Badge key={o} cls={OUTCOME_CLS[o]}>{result.summary[o]} · {t(`omOut_${o}` as TKey)}</Badge>
            ))}
          </div>
          {result.restorePoint && (
            <div style={{ fontSize: 12.5, marginBottom: 8, color: result.restorePoint.ok ? "var(--muted)" : "var(--orange)" }}>
              {t("omRpCreated")}: {result.restorePoint.message}
            </div>
          )}
          {OUTCOME_ORDER.map((o) => {
            const rows = result.results.filter((r) => r.outcome === o);
            if (rows.length === 0) return null;
            return (
              <div key={o} style={{ marginBottom: 10 }}>
                <b style={{ fontSize: 12.5 }}>{t(`omOut_${o}` as TKey)}</b>
                {rows.map((r) => (
                  <div key={r.id} className="finding-row">
                    <Badge cls={OUTCOME_CLS[o]}>{o === "failed" ? "✗" : o === "verified" || o === "already" ? "✓" : "•"}</Badge>
                    <div className="finding-copy"><b>{name(r.id, String(r.name))}</b><span>{engineText(r.detail, lang)}</span></div>
                  </div>
                ))}
              </div>
            );
          })}
          <div className="muted" style={{ fontSize: 12.5, marginTop: 6 }}>{t("omNotMeasured")}</div>
          <div className="row" style={{ gap: 8, marginTop: 10 }}>
            <span className="muted" style={{ fontSize: 12.5 }}>{t("omUndoHint")}</span>
            <button className="btn small ghost" onClick={() => go("reports")}>{t("omOpenHistory")} →</button>
            <button className="btn small ghost" onClick={() => go("benchmark")}>{t("navBenchmark")} →</button>
          </div>
        </Card>
      )}

      {!scan ? (
        !error && <Card><div className="row" style={{ gap: 10 }}><Spinner /><span className="muted">{t("hcScanning")}</span></div></Card>
      ) : (
        <>
          <Card title={t("omPreview")} style={{ marginTop: result ? 14 : 0 }}>
            {plan.length === 0 ? (
              <div className="empty-state"><span>✓</span><b>{t("hcNoRecs")}</b></div>
            ) : (
              <>
                <div className="muted" style={{ fontSize: 12.5, marginBottom: 8 }}><b>{t("omRisks")}:</b> {t("omRisksText")}</div>
                {plan.map((i) => (
                  <label key={i.id} className="tweak" style={{ display: "flex", gap: 10, alignItems: "flex-start", cursor: "pointer" }}>
                    <input
                      type="checkbox"
                      checked={selected.has(i.id)}
                      disabled={applying}
                      onChange={(e) => {
                        const next = new Set(selected);
                        if (e.target.checked) next.add(i.id); else next.delete(i.id);
                        setSelected(next);
                      }}
                      style={{ marginTop: 3 }}
                    />
                    <div style={{ flex: 1 }}>
                      <div className="tweak-head" style={{ marginBottom: 2 }}>
                        <span className="tweak-name">{name(i.id, i.name)}</span>
                        <Badge cls="st-partial">{t(`hcSt_${i.status}` as TKey)}</Badge>
                        <Badge cls={`risk-${i.risk}`}>{i.risk === "Low" ? t("riskLow") : i.risk === "Medium" ? t("riskMedium") : t("riskHigh")}</Badge>
                        {i.reboot !== "notNeeded" && <Badge cls="st-partial">{t(`hcReboot_${i.reboot}` as TKey)}</Badge>}
                      </div>
                      <div className="tweak-desc">{engineText(i.benefit, lang)}</div>
                      <div className="muted" style={{ fontSize: 12 }}>{engineText(i.when, lang)}</div>
                    </div>
                  </label>
                ))}
                <label className="row" style={{ gap: 8, marginTop: 10, fontSize: 13, opacity: admin ? 1 : 0.5 }}>
                  <input type="checkbox" checked={restorePoint && admin} disabled={!admin || applying} onChange={(e) => setRestorePoint(e.target.checked)} />
                  {t("omRestorePoint")}{!admin && ` (${t("hcAdminNeeded")})`}
                </label>
                <div className="row" style={{ gap: 10, marginTop: 12 }}>
                  <button className="btn" disabled={applying || busy || selected.size === 0} onClick={approve}>
                    {applying ? <><Spinner /> {t("omApplying")}</> : `${t("omApprove")} (${selected.size})`}
                  </button>
                  <button className="btn ghost" disabled={applying || busy} onClick={rescan}>{busy ? <Spinner /> : `↻ ${t("hcScan")}`}</button>
                </div>
              </>
            )}
          </Card>

          {blocked.length > 0 && (
            <Card title={t("omNotIncluded")} style={{ marginTop: 14 }}>
              {blocked.map((i) => (
                <div key={i.id} className="finding-row">
                  <Badge cls="st-unknown">–</Badge>
                  <div className="finding-copy"><b>{name(i.id, i.name)}</b><span>{engineText(i.oneClickBlocked, lang)}</span></div>
                </div>
              ))}
            </Card>
          )}

          {manual.length > 0 && (
            <Card title={t("omOtherRecs")} style={{ marginTop: 14 }}>
              <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>{t("omOtherRecsHint")}</div>
              {manual.map((i) => <OptCard key={i.id} item={i} admin={scan.admin} />)}
            </Card>
          )}
        </>
      )}
    </>
  );
}
