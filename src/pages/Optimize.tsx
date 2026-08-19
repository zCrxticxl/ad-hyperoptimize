import React, { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { Card, Badge, Spinner, ActionBtn } from "../components/ui";
import { RiskBadge, RiskNotice } from "../components/HwWarnings";
import { useHwProfile } from "../hooks/useHwProfile";
import { useLang } from "../i18n";
import { localizeTweak } from "../localize";
import type { Mode } from "../App";

export default function Optimize({ mode, admin, focusId, onSwitchExpert }: { mode: Mode; admin: boolean; focusId?: string; onSwitchExpert?: () => void }) {
  const { t, lang } = useLang();
  const [tweaks, setTweaks] = useState<any[] | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [rpStatus, setRpStatus] = useState("");
  const [confirm, setConfirm] = useState<string | null>(null);
  const [riskAck, setRiskAck] = useState<string | null>(null);
  const profile = useHwProfile();
  const focusHandled = useRef(false);
  const [focusHidden, setFocusHidden] = useState(false);

  const refresh = () => api.listTweaks().then(setTweaks);
  useEffect(() => { refresh(); }, []);

  // Deep-link from the Dashboard: expand + scroll to the relevant tweak once.
  useEffect(() => {
    if (!focusId || focusHandled.current || !tweaks) return;
    if (!tweaks.some((t) => t.id === focusId)) return;
    focusHandled.current = true;
    // Beginner mode filters out non-Low tweaks — surface an honest hint instead
    // of a silent no-op when the linked tweak is not visible here.
    if (mode === "beginner" && !tweaks.some((t) => t.id === focusId && t.risk === "Low")) {
      setFocusHidden(true);
      return;
    }
    setOpen(focusId);
    requestAnimationFrame(() => {
      const el = document.querySelector(`[data-tweak-id="${focusId}"]`);
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
      el?.classList.add("tweak-focus");
      setTimeout(() => el?.classList.remove("tweak-focus"), 2200);
    });
  }, [focusId, tweaks]);

  const interp = (tpl: string, params: Record<string, string>) => {
    let s = tpl;
    for (const [k, v] of Object.entries(params)) s = s.replaceAll(`{${k}}`, v);
    return s;
  };

  const push = (m: string) => setLog((l) => [...l.slice(-200), `[${new Date().toLocaleTimeString()}] ${m}`]);

  const visible = (tweaks ?? []).filter((t) => mode === "expert" || t.risk === "Low");
  const cats = [...new Set(visible.map((t) => t.category))];

  const doApply = async (t: any) => {
    push(interp(t("optApplyingLog"), { name: t.name }));
    try {
      await api.applyTweak(t.id);
      push(interp(t("optAppliedLog"), { name: t.name }));
    } catch (e: any) {
      push(interp(t("optErrLog"), { name: t.name, err: String(e) }));
    }
    setConfirm(null);
    setRiskAck(null);
    refresh();
  };

  const doRevert = async (t: any) => {
    const label = t.undoable ? t("optRevertingLog") : t("optForceResetLog");
    push(interp(label, { name: t.name }));
    try {
      const res = await api.revertTweak(t.id);
      if ((res as any)?.journaled === false) {
        push(interp(t("optForceResetDone"), { name: t.name }));
      } else {
        push(interp(t("optRevertedDone"), { name: t.name }));
      }
    } catch (e: any) {
      push(interp(t("optErrLog"), { name: t.name, err: String(e) }));
    }
    refresh();
  };

  const statusLabel = (status: string) =>
    status === "partial" ? t("optStatusPartial")
    : status === "not_applied" ? t("optStatusNotApplied")
    : status === "applied" ? t("optStatusApplied")
    : t("optStatusUnknown");

  return (
    <>
      <h1 className="page-title">{t("optTitle")}</h1>
      <div className="page-sub">
        {t("optSub")}
        {mode === "beginner" && ` ${t("optSubBeginner")}`}
      </div>

      <Card title={t("optSafetyTitle")}>
        <div className="row">
          <ActionBtn
            label={t("optCreateRestorePoint")}
            onRun={async () => {
              try {
                setRpStatus(await api.createRestorePoint("AD HyperOptimize — before optimization"));
              } catch (e: any) {
                setRpStatus(String(e));
              }
            }}
          />
          <span className="muted">{rpStatus || t("optRestoreHint")}</span>
        </div>
      </Card>

      {focusHidden && (
        <div className="warn-banner" style={{ marginTop: 14 }}>
          <b>ℹ </b>{t("optFocusHidden")}{" "}
          <button className="btn small" style={{ marginLeft: 8 }} onClick={() => onSwitchExpert?.()}>{t("modeSwitchExpert")}</button>
        </div>
      )}

      {!tweaks && <div className="mt"><Spinner /> <span className="muted">{t("optReadingState")}</span></div>}

      {cats.map((cat) => (
        <div key={cat} className="mt">
          <h2 style={{ color: "var(--muted)", textTransform: "uppercase", fontSize: 12, letterSpacing: ".5px", marginBottom: 8 }}>{cat}</h2>
          {visible.filter((t) => t.category === cat).map((t) => {
            t = localizeTweak(t, lang);
            const hwRisk = profile?.tweakRisks?.[t.id];
            const needsAck = hwRisk?.severity === "danger" && riskAck !== t.id;
            // "partial" = the tweak was applied but not every value reads back
            // identical (common for GameConfigStore/DWM keys that only settle
            // after a reboot). It must NOT present as un-applied, otherwise the
            // primary Apply button looks permanently stuck. Offer a quiet
            // Re-apply instead and keep Undo available.
            const applyTrigger =
              t.status === "partial"
                ? { idle: t("optReapply"), confirmLabel: t("optConfirmReapply"), cls: "btn small ghost", title: t("optPartialTooltip") }
                : (t.status === "not_applied" || t.status === "unknown")
                ? { idle: t("optApply"), confirmLabel: t("optConfirmApply"), cls: "btn small", title: undefined as string | undefined }
                : null;
            return (
            <div className="tweak" key={t.id} data-tweak-id={t.id}>
              <div className="tweak-head">
                <span className="tweak-name">{t.name}</span>
                <Badge cls={`risk-${t.risk}`}>{t.risk === "Low" ? t("riskLow") : t.risk === "Medium" ? t("riskMedium") : t("riskHigh")}</Badge>
                <RiskBadge id={t.id} />
                <Badge cls={`st-${t.status}`}>{statusLabel(t.status)}</Badge>
                {t.requiresAdmin && <Badge cls="st-unknown">{t("optAdmin")}</Badge>}
                <button className="btn small ghost" aria-expanded={open === t.id} onClick={() => setOpen(open === t.id ? null : t.id)}>
                  {open === t.id ? t("optHide") : t("optDetails")}
                </button>
                {applyTrigger && (
                  confirm === t.id ? (
                    needsAck ? (
                      <button className="btn small ghost" onClick={() => { setConfirm(null); setRiskAck(null); }}>{t("cancel")}</button>
                    ) : (
                      <>
                        <button className="btn small danger" onClick={() => doApply(t)}>{applyTrigger.confirmLabel}</button>
                        <button className="btn small ghost" onClick={() => { setConfirm(null); setRiskAck(null); }}>{t("cancel")}</button>
                      </>
                    )
                  ) : (
                    <button
                      className={applyTrigger.cls}
                      disabled={t.requiresAdmin && !admin}
                      title={applyTrigger.title}
                      onClick={() => { setOpen(t.id); setConfirm(t.id); }}
                    >
                      {applyTrigger.idle}
                    </button>
                  )
                )}
                {t.canUndo && (
                  <button
                    className="btn small ghost"
                    title={t.undoable ? t("optUndoTooltip") : t("optResetTooltip")}
                    onClick={() => doRevert(t)}
                  >
                    {t.undoable ? t("optUndo") : t("optReset")}
                  </button>
                )}
              </div>
              <div className="tweak-desc">{t.description}</div>
              {open === t.id && (
                <div className="tweak-detail">
                  <b>{t("optWhy")}</b> {t.rationale}<br />
                  <b>{t("optImpact")}</b> {t.impact}<br />
                  <b>{t("optRiskLabel")}</b> {t.risk} · <b>{t("optReversibleLabel")}</b> {t.reversible ? t("optReversibleYes") : t("optReversibleNo")}<br />
                  <RiskNotice id={t.id} />
                  {confirm === t.id && needsAck && (
                    <div style={{ marginTop: 8 }}>
                      <button className="btn small danger" onClick={() => setRiskAck(t.id)}>
                        {t("optRiskAck")}
                      </button>
                    </div>
                  )}
                  {confirm === t.id && !needsAck && (
                    <span style={{ color: "var(--yellow)" }}>
                      {t("optConfirmHint")}
                    </span>
                  )}
                </div>
              )}
            </div>
            );
          })}
        </div>
      ))}

      <Card title={t("optLiveLog")} style={{ marginTop: 14 }}>
        <div className="log-console">
          {log.length === 0 ? <span className="muted">{t("optNoActions")}</span> : log.map((l, i) => <div key={i}>{l}</div>)}
        </div>
      </Card>
    </>
  );
}
