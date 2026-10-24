import React, { useState } from "react";
import { api } from "../api";
import { Badge, Switch } from "./ui";
import { RiskBadge, RiskNotice } from "./HwWarnings";
import { useLang, TKey } from "../i18n";
import { localizeTweak } from "../localize";
import { rescanHealth } from "../hooks/useHealthScan";
import { refreshBroker, useBroker } from "../hooks/useBroker";
import { HealthItem, OFFERABLE } from "../lib/health";
import { engineText } from "../lib/engineText";

const EVIDENCE_CLS: Record<string, string> = {
  provenBeneficial: "st-applied",
  conditionallyBeneficial: "st-applied",
  systemMaintenance: "st-unknown",
  noProvenBenefit: "st-unsupported",
  outdated: "st-unsupported",
  broken: "risk-High",
  potentiallyHarmful: "risk-High",
  unverified: "st-partial",
};

const DIAG_CLS: Record<string, string> = {
  configured: "st-applied",
  recommended: "st-partial",
  review: "risk-High",
  drifted: "risk-High",
  failed: "risk-High",
};

/** One optimization: what it does, its honest classification, its real state,
 * and Apply/Undo. Apply re-reads Windows afterwards and only reports success
 * when the setting is confirmed there. */
export function OptCard({ item, admin, focused }: { item: HealthItem; admin: boolean; focused?: boolean }) {
  const { t, lang } = useLang();
  const [open, setOpen] = useState(!!focused);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const tx = (s: string | null | undefined) => engineText(s, lang);
  const loc = localizeTweak({ id: item.id, name: item.name, description: item.description }, lang);

  const applied = item.status === "applied" || item.status === "partial";
  const offerable = OFFERABLE.includes(item.evidence);
  const harmfulish = ["potentiallyHarmful", "outdated", "broken"].includes(item.evidence);
  const canApply =
    !applied &&
    !harmfulish &&
    item.status !== "unsupported" &&
    !["notApplicable", "drifted", "failed", "redundant", "configured", "retired", "retiredActive"].includes(item.diagnosis);
  const broker = useBroker();
  // With the admin helper connected, Apply/Undo run elevated through it.
  const adminBlocked = item.requiresAdmin && !admin && !broker.connected;
  // Drifted/failed items are no longer (fully) active but still hold an open
  // journal entry: Undo restores the captured originals and closes it, so
  // they must offer it too, or the card is a dead end.
  const undoable = item.canUndo && (applied || ["drifted", "failed"].includes(item.diagnosis));

  const run = async (kind: "apply" | "undo") => {
    if (busy) return;
    if (kind === "apply" && !window.confirm(offerable ? t("hcConfirmApply") : t("hcConfirmNoBenefit"))) return;
    setBusy(true);
    setMsg(null);
    try {
      if (kind === "apply") await api.applyTweak(item.id);
      else await api.revertTweak(item.id);
      const scan = await rescanHealth();
      const after = scan.items.find((i) => i.id === item.id);
      if (kind === "apply") {
        const confirmed = after?.status === "applied" && after.statusSource === "live";
        setMsg({ ok: confirmed, text: confirmed ? t("hcApplied") : t("hcApplyNotConfirmed") });
      }
    } catch (e) {
      setMsg({ ok: false, text: String(e) });
      refreshBroker(); // the helper may have ended (idle timeout)
    } finally {
      setBusy(false);
    }
  };

  const reboot = item.reboot !== "notNeeded" ? t(`hcReboot_${item.reboot}` as TKey) : null;

  return (
    <div className={`tweak ${focused ? "feature-focus" : ""}`} data-focus-id={item.id}>
      <div className="tweak-head">
        <span className="tweak-name">{loc.name}</span>
        <Badge cls={DIAG_CLS[item.diagnosis] ?? "st-unknown"}>{t(`hcDiag_${item.diagnosis}` as TKey)}</Badge>
        <Badge cls={EVIDENCE_CLS[item.evidence] ?? "st-unknown"}>{t(`hcEv_${item.evidence}` as TKey)}</Badge>
        {reboot && <Badge cls="st-partial">{reboot}</Badge>}
        <RiskBadge id={item.id} />
        <span style={{ marginLeft: "auto", display: "flex", gap: 6 }}>
          <button className="btn small ghost" onClick={() => setOpen(!open)} aria-expanded={open}>
            {open ? "▲" : "▼"}
          </button>
          {(undoable || canApply) && (
            // Drifted/failed: no longer fully active but an open entry
            // remains, shown as partial so switching off runs the undo.
            <Switch
              on={applied || undoable}
              partial={item.status === "partial" || (!applied && undoable)}
              label={loc.name}
              title={adminBlocked ? t("hcAdminNeeded") : applied || undoable ? t("hcUndo") : t("hcApply")}
              busy={busy}
              disabled={adminBlocked || (applied || undoable ? !undoable : !canApply)}
              onChange={(next) => run(next ? "apply" : "undo")}
            />
          )}
        </span>
      </div>
      <div className="tweak-desc">{tx(item.benefit)}</div>
      {item.reason && item.reason !== item.when && <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>{tx(item.reason)}</div>}
      {msg && <div style={{ fontSize: 12, marginTop: 6, color: msg.ok ? "var(--green)" : "var(--red)" }}>{msg.text}</div>}
      {open && (
        <div className="tweak-detail" style={{ display: "grid", gridTemplateColumns: "max-content 1fr", gap: "4px 12px", fontSize: 12.5, marginTop: 8 }}>
          <span className="muted">{t("hcState")}</span>
          <span>{t(`hcSt_${item.status}` as TKey)} · <span className="muted">{t(`hcSrc_${item.statusSource}` as TKey)}</span></span>
          <span className="muted">{t("hcBenefit")}</span>
          <span>{tx(item.benefit)}</span>
          {item.when && <><span className="muted">{t("hcWhen")}</span><span>{tx(item.when)}</span></>}
          <span className="muted">{t("hcRisk")}</span>
          <span>{item.risk === "Low" ? t("riskLow") : item.risk === "Medium" ? t("riskMedium") : t("riskHigh")}{adminBlocked ? ` · ${t("hcAdminNeeded")}` : ""}</span>
          <span className="muted">{t("hcRestore")}</span>
          <span>{undoable ? t("hcRestoreUndo") : applied ? "–" : t("hcRestoreAfter")}</span>
          <span className="muted">{t("hcColName")}</span>
          <span className="muted">{loc.description}</span>
          <span /><RiskNotice id={item.id} />
          {!canApply && !applied && harmfulish && <><span className="muted">{t("hcNotOffered")}</span><span>{tx(item.when)}</span></>}
        </div>
      )}
    </div>
  );
}
