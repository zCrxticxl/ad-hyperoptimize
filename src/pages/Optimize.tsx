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
    if (!tweaks.some((tw) => tw.id === focusId)) return;
    focusHandled.current = true;
    // Beginner mode filters out non-Low tweaks, surface an honest hint instead
    // of a silent no-op when the linked tweak is not visible here.
    if (mode === "beginner" && !tweaks.some((tw) => tw.id === focusId && tw.risk === "Low")) {
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

  const visible = (tweaks ?? []).filter((tw) => mode === "expert" || tw.risk === "Low");
  const cats = [...new Set(visible.map((tw) => tw.category))];

  const doApply = async (tw: any) => {
    push(interp(t("optApplyingLog"), { name: tw.name }));
    try {
      await api.applyTweak(tw.id);
      push(interp(t("optAppliedLog"), { name: tw.name }));
    } catch (e: any) {
      push(interp(t("optErrLog"), { name: tw.name, err: String(e) }));
    }
    setConfirm(null);
    setRiskAck(null);
    refresh();
  };

  const doRevert = async (tw: any) => {
    const label = tw.undoable ? t("optRevertingLog") : t("optForceResetLog");
    push(interp(label, { name: tw.name }));
    try {
      const res = await api.revertTweak(tw.id);
      if ((res as any)?.journaled === false) {
        push(interp(t("optForceResetDone"), { name: tw.name }));
      } else {
        push(interp(t("optRevertedDone"), { name: tw.name }));
      }
    } catch (e: any) {
      push(interp(t("optErrLog"), { name: tw.name, err: String(e) }));
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
                setRpStatus(await api.createRestorePoint("AD HyperOptimize, before optimization"));
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
          {visible.filter((tw) => tw.category === cat).map((tw) => {
            tw = localizeTweak(tw, lang);
            const hwRisk = profile?.tweakRisks?.[tw.id];
            const needsAck = hwRisk?.severity === "danger" && riskAck !== tw.id;
            // Apply is only shown when there is genuinely nothing to undo yet
            // (no journal entry and not detected as applied). Once the tweak
            // is applied / undoable, the primary action is Undo alone — never
            // show Apply next to Undo (confusing dual buttons).
            const applyTrigger =
              !tw.canUndo && (tw.status === "not_applied" || tw.status === "unknown")
                ? { idle: t("optApply"), confirmLabel: t("optConfirmApply"), cls: "btn small", title: undefined as string | undefined }
                : null;
            return (
            <div className="tweak" key={tw.id} data-tweak-id={tw.id}>
              <div className="tweak-head">
                <span className="tweak-name">{tw.name}</span>
                <Badge cls={`risk-${tw.risk}`}>{tw.risk === "Low" ? t("riskLow") : tw.risk === "Medium" ? t("riskMedium") : t("riskHigh")}</Badge>
                <RiskBadge id={tw.id} />
                <Badge cls={`st-${tw.status}`}>{statusLabel(tw.status)}</Badge>
                {tw.requiresAdmin && <Badge cls="st-unknown">{t("optAdmin")}</Badge>}
                <button className="btn small ghost" aria-expanded={open === tw.id} onClick={() => setOpen(open === tw.id ? null : tw.id)}>
                  {open === tw.id ? t("optHide") : t("optDetails")}
                </button>
                {applyTrigger && (
                  confirm === tw.id ? (
                    needsAck ? (
                      <button className="btn small ghost" onClick={() => { setConfirm(null); setRiskAck(null); }}>{t("cancel")}</button>
                    ) : (
                      <>
                        <button className="btn small danger" onClick={() => doApply(tw)}>{applyTrigger.confirmLabel}</button>
                        <button className="btn small ghost" onClick={() => { setConfirm(null); setRiskAck(null); }}>{t("cancel")}</button>
                      </>
                    )
                  ) : (
                    <button
                      className={applyTrigger.cls}
                      disabled={tw.requiresAdmin && !admin}
                      title={applyTrigger.title}
                      onClick={() => { setOpen(tw.id); setConfirm(tw.id); }}
                    >
                      {applyTrigger.idle}
                    </button>
                  )
                )}
                {tw.canUndo && (
                  <button
                    className="btn small ghost"
                    title={tw.undoable ? t("optUndoTooltip") : t("optResetTooltip")}
                    onClick={() => doRevert(tw)}
                  >
                    {tw.undoable ? t("optUndo") : t("optReset")}
                  </button>
                )}
              </div>
              <div className="tweak-desc">{tw.description}</div>
              {open === tw.id && (
                <div className="tweak-detail">
                  <b>{t("optWhy")}</b> {tw.rationale}<br />
                  <b>{t("optImpact")}</b> {tw.impact}<br />
                  <b>{t("optRiskLabel")}</b> {tw.risk} · <b>{t("optReversibleLabel")}</b> {tw.reversible ? t("optReversibleYes") : t("optReversibleNo")}<br />
                  <RiskNotice id={tw.id} />
                  {confirm === tw.id && needsAck && (
                    <div style={{ marginTop: 8 }}>
                      <button className="btn small danger" onClick={() => setRiskAck(tw.id)}>
                        {t("optRiskAck")}
                      </button>
                    </div>
                  )}
                  {confirm === tw.id && !needsAck && (
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
