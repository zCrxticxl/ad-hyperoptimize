import React, { useEffect, useState } from "react";
import { api } from "../api";
import { ActionBtn, Card, ErrorBanner, Spinner } from "../components/ui";
import { OptCard } from "../components/OptCard";
import { RetiredCard } from "../components/RetiredCard";
import { useHealthScan } from "../hooks/useHealthScan";
import { useFeatureFocus } from "../hooks/useFeatureFocus";
import { useLang, TKey } from "../i18n";
import { localizeTweak } from "../localize";
import { HealthItem, NEEDS_REVIEW } from "../lib/health";
import type { Mode } from "../App";

/** Optimize: only optimizations that fit this PC are shown by default,
 * grouped by what they are for. Everything else (no proven benefit,
 * troubleshooting, privacy, not applicable) stays reachable under Advanced
 * with its honest classification. Every state comes from a live scan. */
export default function Optimize({ mode, admin, focusId }: { mode: Mode; admin: boolean; focusId?: string; onSwitchExpert?: () => void }) {
  const { t, lang } = useLang();
  const { scan, busy, error, rescan } = useHealthScan();
  const [showAdvanced, setShowAdvanced] = useState(mode === "expert");
  const [query, setQuery] = useState("");
  const [rpStatus, setRpStatus] = useState("");

  const items = scan?.items ?? [];
  const isRecommended = (i: HealthItem) => i.diagnosis === "recommended";
  const isReview = (i: HealthItem) => NEEDS_REVIEW.includes(i.diagnosis);
  const inGroup = (group: string) => (i: HealthItem) =>
    i.group === group && ["available", "configured"].includes(i.diagnosis);
  // Retired tweaks are never listed: active ones live in the reset card.
  const shown = new Set<string>(items.filter((i) => i.diagnosis === "retired" || i.diagnosis === "retiredActive").map((i) => i.id));
  const take = (pred: (i: HealthItem) => boolean) => {
    const out = items.filter((i) => !shown.has(i.id) && pred(i));
    out.forEach((i) => shown.add(i.id));
    return out;
  };
  const sections: [TKey, HealthItem[]][] = [
    ["optNeedsReview", take(isReview)],
    ["optRecommended", take(isRecommended)],
    ["optGaming", take(inGroup("gaming"))],
    ["optSystem", take(inGroup("system"))],
  ];
  const q = query.trim().toLocaleLowerCase();
  const advanced = items
    .filter((i) => !shown.has(i.id))
    .filter((i) => !q || `${localizeTweak({ id: i.id, name: i.name, description: i.description }, lang).name} ${i.description} ${i.id}`.toLocaleLowerCase().includes(q));

  // A deep link (search, dashboard) to an item that lives under Advanced
  // opens Advanced so the target is visible.
  const focusInAdvanced = !!focusId && advanced.some((i) => i.id === focusId);
  useEffect(() => { if (focusInAdvanced) setShowAdvanced(true); }, [focusInAdvanced]);
  useFeatureFocus(focusId, !!scan);

  return (
    <>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start", flexWrap: "wrap", gap: 12 }}>
        <div>
          <h1 className="page-title">{t("navOptimize")}</h1>
          <div className="page-sub">{t("toolDescOptimize")}</div>
        </div>
        <button className="btn ghost" onClick={rescan} disabled={busy}>{busy ? <><Spinner /> {t("hcScanning")}</> : `↻ ${t("hcScan")}`}</button>
      </div>

      <Card title={t("optSafetyTitle")}>
        <div className="row">
          <ActionBtn
            label={t("optCreateRestorePoint")}
            disabled={!admin}
            onRun={async () => {
              try {
                setRpStatus(await api.createRestorePoint("AD HyperOptimize, before optimization"));
              } catch (e: any) {
                setRpStatus(String(e));
              }
            }}
          />
          <span className="muted">{rpStatus || t("optRestoreHint")}</span>
        </div>
      </Card>

      {error && <ErrorBanner msg={error} onRetry={rescan} />}
      {!scan && !error && <div className="mt"><Spinner /> <span className="muted">{t("hcScanning")}</span></div>}

      {scan && <RetiredCard scan={scan} admin={admin} />}

      {scan && sections.every(([, list]) => list.length === 0) && (
        <Card style={{ marginTop: 14 }}><div className="empty-state"><span>✓</span><b>{t("hcNoRecs")}</b></div></Card>
      )}

      {sections.map(([title, list]) => list.length > 0 && (
        <div key={title} className="mt">
          <h2 className="section-label">{t(title)}</h2>
          {list.map((i) => <OptCard key={i.id} item={i} admin={admin} focused={i.id === focusId} />)}
        </div>
      ))}

      {scan && (
        <div className="mt">
          <div className="row" style={{ justifyContent: "space-between", flexWrap: "wrap", gap: 8 }}>
            <h2 className="section-label" style={{ margin: 0 }}>{t("optAdvanced")} ({items.filter((i) => !shown.has(i.id)).length})</h2>
            <button className="btn small ghost" onClick={() => setShowAdvanced(!showAdvanced)} aria-expanded={showAdvanced}>
              {showAdvanced ? t("optHideAdvanced") : t("optShowAdvanced")}
            </button>
          </div>
          <div className="muted" style={{ fontSize: 12, margin: "6px 0 10px" }}>{t("optAdvancedHint")}</div>
          {showAdvanced && (
            <>
              <input className="input" value={query} onChange={(e) => setQuery(e.target.value)} placeholder={t("optSearch")}
                aria-label={t("optSearch")} style={{ width: "100%", maxWidth: 360, marginBottom: 10 }} />
              {advanced.map((i) => <OptCard key={i.id} item={i} admin={admin} focused={i.id === focusId} />)}
            </>
          )}
        </div>
      )}
    </>
  );
}
