import React, { useState } from "react";
import { api } from "../api";
import { Badge, Card, ErrorBanner, Spinner } from "./ui";
import { OptCard } from "./OptCard";
import { rescanHealth } from "../hooks/useHealthScan";
import { useLang, TKey } from "../i18n";
import { localizeTweak } from "../localize";
import type { ApplyResult, HealthScan } from "../lib/health";
import { engineText } from "../lib/engineText";

/** Retired tweaks (no measurable benefit) that are still active on this PC,
 * with one button that resets all of them to their original values and
 * reports, per item, whether Windows confirms the reset. Renders nothing
 * when none are active, so retired tweaks then disappear entirely. */
export function RetiredCard({ scan, admin }: { scan: HealthScan; admin: boolean }) {
  const { t, lang } = useLang();
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<ApplyResult | null>(null);
  const [err, setErr] = useState("");
  const active = scan.items.filter((i) => i.diagnosis === "retiredActive");
  if (active.length === 0 && !result) return null;

  const resetAll = async () => {
    if (busy || !window.confirm(t("retConfirm").replace("{n}", String(active.length)))) return;
    setBusy(true);
    setErr("");
    try {
      const r: ApplyResult = await api.resetRetired();
      if (r.error) setErr(r.error);
      else setResult(r);
    } catch (e) {
      setErr(String(e));
    } finally {
      await rescanHealth().catch(() => {});
      setBusy(false);
    }
  };

  return (
    <Card title={`${t("retTitle")} (${active.length})`} style={{ marginTop: 14 }}>
      <div className="muted" style={{ fontSize: 12.5, marginBottom: 10 }}>{t("retHint")}</div>
      {err && <ErrorBanner msg={err} />}
      {result && (
        <div style={{ marginBottom: 10 }}>
          {result.results.map((r) => (
            <div key={r.id} className="finding-row">
              <Badge cls={r.outcome === "verified" ? "st-applied" : r.outcome === "failed" ? "risk-High" : "st-partial"}>
                {t(`omOut_${r.outcome}` as TKey)}
              </Badge>
              <div className="finding-copy">
                <b>{localizeTweak({ id: r.id, name: String(r.name), description: "" }, lang).name}</b>
                <span>{engineText(r.detail, lang)}</span>
              </div>
            </div>
          ))}
        </div>
      )}
      {active.length > 0 && (
        <>
          <button className="btn" disabled={busy} onClick={resetAll}>
            {busy ? <><Spinner /> {t("retResetting")}</> : `${t("retResetAll")} (${active.length})`}
          </button>
          {!admin && active.some((i) => i.requiresAdmin) && (
            <span className="muted" style={{ fontSize: 12, marginLeft: 10 }}>{t("retAdminHint")}</span>
          )}
          <details style={{ marginTop: 10 }}>
            <summary style={{ cursor: "pointer", fontSize: 12.5 }}>{t("retShowItems")}</summary>
            {active.map((i) => <OptCard key={i.id} item={i} admin={admin} />)}
          </details>
        </>
      )}
    </Card>
  );
}
