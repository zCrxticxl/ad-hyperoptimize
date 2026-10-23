import React from "react";
import { api, fmtAge } from "../api";
import { Badge, Card, ErrorBanner, Spinner } from "../components/ui";
import { OptCard } from "../components/OptCard";
import { RetiredCard } from "../components/RetiredCard";
import { useHealthScan } from "../hooks/useHealthScan";
import { useLang, TKey } from "../i18n";
import { NEEDS_REVIEW, planName } from "../lib/health";
import type { Mode } from "../App";
import { engineText } from "../lib/engineText";

export default function SystemCheck({ mode, go }: { mode: Mode; go: (id: string, target?: string) => void }) {
  const { t, lang } = useLang();
  const { scan, busy, error, rescan } = useHealthScan();

  if (!scan) {
    return (
      <>
        <h1 className="page-title">{t("hcTitle")}</h1>
        <div className="page-sub">{t("hcSub")}</div>
        {error ? <ErrorBanner msg={error} onRetry={rescan} /> : <Card><div className="row" style={{ gap: 10 }}><Spinner /><span className="muted">{t("hcScanning")}</span></div></Card>}
      </>
    );
  }

  const s = scan.system;
  const recs = scan.items.filter((i) => i.diagnosis === "recommended");
  const review = scan.items.filter((i) => NEEDS_REVIEW.includes(i.diagnosis));
  const tiles: [TKey, number, string][] = [
    ["hcCntConfigured", scan.summary.configured, "green"],
    ["hcCntRecommended", scan.summary.recommended, "violet"],
    ["hcCntReview", scan.summary.review + scan.summary.retiredActive, scan.summary.review + scan.summary.retiredActive ? "red" : "green"],
    ["hcCntNotApplicable", scan.summary.notApplicable, "blue"],
    ["hcCntFailed", scan.summary.failed, scan.summary.failed ? "red" : "green"],
    ["hcCntAttention", scan.summary.attention, scan.summary.attention ? "red" : "green"],
  ];
  const gpu = s.gpus.find((g) => g.dedicated) ?? s.gpus[0];

  return (
    <>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap", gap: 12 }}>
        <div>
          <h1 className="page-title">{t("hcTitle")}</h1>
          <div className="page-sub">{t("hcSub")}</div>
        </div>
        <div className="row" style={{ gap: 10 }}>
          <span className="muted" style={{ fontSize: 12 }}>{t("hcScanned")} {fmtAge(scan.time, lang)}</span>
          <button className="btn ghost" onClick={rescan} disabled={busy}>{busy ? <><Spinner /> {t("hcScanning")}</> : `↻ ${t("hcScan")}`}</button>
          {recs.some((r) => r.oneClick) && <button className="btn" onClick={() => go("autoopt")}>⚡ {t("hcGoOptimize")}</button>}
        </div>
      </div>
      {error && <ErrorBanner msg={error} onRetry={rescan} />}

      <Card title={t("hcSystem")}>
        <div style={{ display: "grid", gridTemplateColumns: "max-content 1fr", gap: "6px 16px", fontSize: 13 }}>
          <span className="muted">{t("hcWindows")}</span>
          <span>{s.windows.isWindows11 ? "Windows 11" : "Windows 10"} {s.windows.version} · Build {s.windows.build}.{s.windows.ubr} · {s.windows.edition}</span>
          <span className="muted">{t("hcCpu")}</span>
          <span>
            {s.cpu.name} · {s.cpu.cores}C/{s.cpu.threads}T
            {s.cpu.hybrid && <> · <Badge cls="st-unknown">{t("hcHybrid")}</Badge></>}
            {s.cpu.dualCcdX3d && <> · <Badge cls="st-unknown">{t("hcX3d")}</Badge></>}
          </span>
          <span className="muted">{t("hcGpu")}</span>
          <span>{gpu ? `${gpu.name} · ${t("hcDriver")} ${gpu.driver}${gpu.driverDate ? ` (${gpu.driverDate})` : ""}` : "–"}</span>
          <span className="muted">{t("hcRam")}</span>
          <span>{(s.ramMb / 1024).toFixed(0)} GB</span>
          <span className="muted">{t("hcStorage")}</span>
          <span>
            {s.storage.systemFreePct != null ? `${s.storage.systemFreePct.toFixed(1)} % ${t("hcFree")}` : "–"}
            {s.storage.hasHdd && ` · ${t("hcHddPresent")}`}
          </span>
          <span className="muted">{t("hcPower")}</span>
          <span>{planName(scan)}</span>
          <span className="muted">{t("hcDevice")}</span>
          <span>{s.isLaptop ? t("hcLaptop") : t("hcDesktop")}</span>
        </div>
        {(s.cpu.hybrid || s.cpu.dualCcdX3d) && <div className="muted" style={{ fontSize: 11.5, marginTop: 8 }}>{t("hcModelHint")}</div>}
      </Card>

      <Card title={t("hcStatusTitle")} style={{ marginTop: 14 }}>
        <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>{t("hcStatusDef")}</div>
        <div className="grid-3">
          {tiles.map(([label, n, tone]) => (
            <div key={label} className="overview-stat">
              <span className={`overview-icon ${tone}`}>{n}</span>
              <div><b>{n}</b><span>{t(label)}</span></div>
            </div>
          ))}
        </div>
      </Card>

      <Card title={t("gsTitle")} style={{ marginTop: 14 }}>
        <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>{t("gsHint")}</div>
        {(scan.gaming ?? []).map((g) => (
          <div key={g.id} className="finding-row">
            <Badge cls={g.status === "ok" ? "st-applied" : g.status === "warning" ? "risk-High" : g.status === "info" ? "st-partial" : "st-unknown"}>
              {g.status === "ok" ? "✓" : g.status === "warning" ? "!" : g.status === "info" ? "i" : "?"} {t(`gsStatus_${g.status}` as TKey)}
            </Badge>
            <div className="finding-copy"><b>{engineText(g.title, lang)}</b><span>{engineText(g.detail, lang)}</span></div>
            {g.settings && (
              <button className="row-action" onClick={() => api.openWindowsSettings(g.settings!).catch(() => {})}>
                {t("gsOpenSettings")} →
              </button>
            )}
          </div>
        ))}
      </Card>

      {scan.checks.length > 0 && (
        <Card title={t("hcAttention")} style={{ marginTop: 14 }}>
          {scan.checks.map((c) => (
            <div key={c.id} className="finding-row">
              <Badge cls={c.severity === "warning" ? "risk-High" : "st-unknown"}>{c.severity === "warning" ? "!" : "i"}</Badge>
              <div className="finding-copy"><b>{engineText(c.title, lang)}</b><span>{engineText(c.detail, lang)}</span></div>
              {c.tool && <button className="row-action" onClick={() => go(c.tool!)}>{t("hcOpenTool")} →</button>}
            </div>
          ))}
        </Card>
      )}

      <Card title={t("hcRecommendations")} style={{ marginTop: 14 }}>
        {recs.length === 0
          ? <div className="empty-state"><span>✓</span><b>{t("hcNoRecs")}</b></div>
          : recs.map((i) => <OptCard key={i.id} item={i} admin={scan.admin} />)}
      </Card>

      <RetiredCard scan={scan} admin={scan.admin} />

      {review.length > 0 && (
        <Card title={t("hcReviewTitle")} style={{ marginTop: 14 }}>
          <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>{t("hcReviewHint")}</div>
          {review.map((i) => <OptCard key={i.id} item={i} admin={scan.admin} />)}
        </Card>
      )}

      <details style={{ marginTop: 14 }} open={mode === "expert"}>
        <summary style={{ cursor: "pointer", fontWeight: 600 }}>{t("hcDetails")}</summary>
        <div className="muted" style={{ fontSize: 12, margin: "6px 0 8px" }}>{t("hcDetailsHint")}</div>
        <table className="tbl">
          <thead><tr><th>{t("hcColName")}</th><th>{t("hcColEvidence")}</th><th>{t("hcColDiagnosis")}</th><th>{t("hcColState")}</th></tr></thead>
          <tbody>
            {scan.items.filter((i) => i.diagnosis !== "retired").map((i) => (
              <tr key={i.id} title={engineText(i.reason, lang)}>
                <td>{i.name}</td>
                <td>{t(`hcEv_${i.evidence}` as TKey)}</td>
                <td>{t(`hcDiag_${i.diagnosis}` as TKey)}</td>
                <td>{t(`hcSt_${i.status}` as TKey)} <span className="muted">({t(`hcSrc_${i.statusSource}` as TKey)})</span></td>
              </tr>
            ))}
          </tbody>
        </table>
      </details>
    </>
  );
}
