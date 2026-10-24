import React, { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Card, Spinner, Badge, Switch } from "../components/ui";
import { useLang, TKey } from "../i18n";
import { localizeCatalog } from "../localize";
import { useFeatureFocus } from "../hooks/useFeatureFocus";

type PTweak = {
  id: string; name: string; category: string;
  description: string; risk: "Low" | "Medium" | "High"; applied: boolean; canRevert: boolean;
  retiredReason?: string | null;
};

const RISK_CLS: Record<string, string> = { Low: "risk-Low", Medium: "risk-Medium", High: "risk-High" };

// Keys are the backend category names (privacy.rs, English); icons and display
// labels are translated here.
const CAT_ICONS: Record<string, string> = {
  "Telemetry": "📡",
  "Advertising": "📢",
  "Search": "🔍",
  "Activity History": "📋",
  "Input": "⌨",
  "Location": "📍",
  "Feedback": "💬",
  "Network": "🌐",
};

export default function PrivacyCenter({ admin, focusId }: { admin: boolean; focusId?: string }) {
  const { t, lang } = useLang();

  // Translate backend category names (privacy.rs returns English names).
  const CAT_LABEL: Record<string, string> = {
    "Telemetry":        t("catTelemetrie"),
    "Advertising":      t("catWerbung"),
    "Search":           t("catSuche"),
    "Activity History": t("catAktivitaet"),
    "Input":            t("catEingabe"),
    "Location":         t("catStandort"),
    "Feedback":         t("catFeedback"),
    "Network":          t("catNetzwerk"),
  };
  const [tweaks, setTweaks]   = useState<PTweak[]>([]);
  // Retired items, only present while still active (backend filters).
  const [retired, setRetired] = useState<PTweak[]>([]);
  const [applied, setApplied] = useState(0);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy]       = useState<string | null>(null);
  const [log, setLog]         = useState<string[]>([]);
  const [err, setErr]         = useState("");
  const [filter, setFilter]   = useState<string | null>(null);
  useFeatureFocus(focusId, !loading);
  const [openId, setOpenId]   = useState<string | null>(null);

  // Localized reason; the backend text is the fallback for unknown ids.
  const reason = (id: string, fallback?: string | null) => {
    const key = `wdReason_${id}`;
    const text = t(key as TKey);
    return text === key ? fallback ?? "" : text;
  };

  const push = (m: string) =>
    setLog(l => [`[${new Date().toLocaleTimeString()}] ${m}`, ...l.slice(0, 49)]);
  const fmt = (key: Parameters<typeof t>[0], name: string, warning = "") =>
    t(key).replace("{name}", name).replace("{warning}", warning);

  const refresh = () => {
    setLoading(true);
    api.privacyScan()
      .then((r: any) => {
        if (r.error) throw new Error(String(r.error));
        setErr("");
        const loc = (tw: PTweak): PTweak => ({ ...tw, ...localizeCatalog("privacy", tw.id, tw.name, tw.description, lang) });
        setTweaks((r.tweaks ?? []).map(loc));
        setRetired((r.retired ?? []).map(loc));
        setApplied(r.applied ?? 0);
      })
      .catch(e => setErr(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => { refresh(); }, [lang]);

  const categories = useMemo(() => [...new Set(tweaks.map(t => t.category))], [tweaks]);

  const visibleTweaks = useMemo(() =>
    filter ? tweaks.filter(t => t.category === filter) : tweaks,
    [tweaks, filter]);

  const doApply = async (tw: PTweak, refreshAfter = true) => {
    if (!admin || busy) return;
    setBusy(tw.id);
    push(fmt("privLogApplying", tw.name));
    try {
      const res = await api.privacyApply(tw.id);
      if (res?.ok === false) push(`✘ ${fmt("privLogNotApplied", tw.name, String(res.warning ?? ""))}`);
      else if (res?.warning) push(`⚠ ${fmt("privLogAppliedWarn", tw.name, String(res.warning))}`);
      else push(`✔ ${tw.name} ${t("active")}`);
    } catch (e: any) { push(`✘ ${tw.name}: ${e}`); setErr(String(e)); }
    finally { setBusy(null); if (refreshAfter) refresh(); }
  };

  const doRevert = async (tw: PTweak) => {
    if (!admin || busy) return;
    setBusy(tw.id);
    push(fmt("privLogReverting", tw.name));
    try {
      const res = await api.privacyRevert(tw.id);
      if (res?.ok === false) push(`✘ ${fmt("privLogNotReverted", tw.name, String(res.warning ?? ""))}`);
      else if (res?.warning) push(`⚠ ${fmt("privLogRevertedWarn", tw.name, String(res.warning))}`);
      else push(`↩ ${fmt("privLogReverted", tw.name)}`);
    } catch (e: any) { push(`✘ ${tw.name}: ${e}`); setErr(String(e)); }
    finally { setBusy(null); refresh(); }
  };

  const applyAll = async () => {
    if (!admin || busy) return;
    const pending = tweaks.filter(t => !t.applied);
    if (!pending.length) return;
    const risky = pending.filter(tw => tw.risk !== "Low");
    let msg = t("privApplyAllConfirm").replace("{n}", String(pending.length));
    if (risky.length) msg += `\n\n${t("privApplyAllRiskList")}\n${risky.map(tw => `• ${tw.name} (${tw.risk})`).join("\n")}`;
    if (!window.confirm(msg)) return;
    setBusy("all");
    let succeeded = 0;
    const failed: string[] = [];
    for (const tw of pending) {
      try {
        const result = await api.privacyApply(tw.id);
        if (result?.ok === false) failed.push(tw.name);
        else succeeded++;
      } catch (error) {
        failed.push(`${tw.name}: ${String(error)}`);
      }
    }
    push(t("privLogSummary").replace("{ok}", String(succeeded)).replace("{failed}", String(failed.length)));
    if (failed.length) setErr(failed.join("; "));
    setBusy(null);
    refresh();
  };

  const totalApplied = tweaks.filter(t => t.applied).length;
  // Share of the catalog that is applied, not a measured privacy score.
  const pct = tweaks.length > 0 ? Math.round(totalApplied / tweaks.length * 100) : 0;

  return (
    <>
      <h1 className="page-title">{t("privTitle")}</h1>
      <div className="page-sub">{t("privSub")}</div>

      {loading && <><Spinner /> <span className="muted">{t("privLoading")}</span></>}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}
      {!admin && <div style={{ color: "var(--yellow)", marginBottom: 10, fontSize: 13 }}>{t("adminRestart")}</div>}

      {!loading && tweaks.length > 0 && (
        <>
          {/* Score card */}
          <Card title="">
            <div style={{ display: "flex", alignItems: "center", gap: 20, flexWrap: "wrap" }}>
              <div style={{ textAlign: "center", minWidth: 90 }}>
                <div style={{
                  fontSize: 36, fontWeight: 900,
                  color: pct >= 80 ? "var(--green)" : pct >= 50 ? "var(--yellow)" : "var(--red)"
                }}>{totalApplied}/{tweaks.length}</div>
                <div className="muted" style={{ fontSize: 11 }}>{t("privScore")}</div>
              </div>
              <div style={{ flex: 1 }}>
                <div style={{ height: 10, background: "var(--border)", borderRadius: 5, overflow: "hidden" }}>
                  <div style={{
                    height: 10, borderRadius: 5, transition: "width .4s",
                    width: `${pct}%`,
                    background: pct >= 80 ? "var(--green)" : pct >= 50 ? "var(--yellow)" : "var(--red)",
                  }} />
                </div>
                <div className="muted" style={{ fontSize: 12, marginTop: 6 }}>
                  {totalApplied} {t("privTweaksOf")} {tweaks.length} {t("privTweaksActive")}
                </div>
              </div>
              <div style={{ display: "flex", gap: 8 }}>
                <button className="btn" disabled={!!busy || !admin || totalApplied === tweaks.length} onClick={applyAll}>
                  {busy === "all" ? <Spinner /> : t("applyAll")}
                </button>
                <button className="btn ghost small" onClick={refresh} disabled={loading || !!busy}>{t("refresh")}</button>
              </div>
            </div>
          </Card>

          {/* Category filter chips */}
          <div style={{ display: "flex", gap: 8, flexWrap: "wrap", margin: "12px 0" }}>
            <button
              className={`btn small ${!filter ? "" : "ghost"}`}
              onClick={() => setFilter(null)}
            >
              {t("privAll")} ({tweaks.length})
            </button>
            {categories.map(cat => {
              const count    = tweaks.filter(t => t.category === cat && t.applied).length;
              const catTotal = tweaks.filter(t => t.category === cat).length;
              return (
                <button
                  key={cat}
                  className={`btn small ${filter === cat ? "" : "ghost"}`}
                  onClick={() => setFilter(f => f === cat ? null : cat)}
                >
                  {CAT_ICONS[cat] ?? "○"} {CAT_LABEL[cat] ?? cat} ({count}/{catTotal})
                </button>
              );
            })}
          </div>

          {/* Tweak list */}
          <Card title={filter ? `${CAT_ICONS[filter] ?? ""} ${CAT_LABEL[filter] ?? filter}` : t("privAllTweaks")}>
            {visibleTweaks.map(tw => {
              const isBusy = busy === tw.id;
              const isOpen = openId === tw.id;
              return (
                <div className="tweak" key={tw.id} data-focus-id={tw.id}>
                  <div className="tweak-head">
                    <span className="tweak-name" style={{ color: tw.applied ? "var(--green)" : undefined }}>
                      {tw.name}
                    </span>
                    <Badge cls={RISK_CLS[tw.risk]}>{tw.risk}</Badge>
                    <button className="btn small ghost" onClick={() => setOpenId(isOpen ? null : tw.id)}>
                      {isOpen ? "▲" : "▼"}
                    </button>
                    <Switch
                      on={tw.applied}
                      label={tw.name}
                      busy={isBusy}
                      disabled={!!busy || !admin || (tw.applied && !tw.canRevert)}
                      onChange={(next) => (next ? doApply(tw) : doRevert(tw))}
                    />
                  </div>
                  {isOpen && (
                    <div className="tweak-detail">
                      <p>{tw.description}</p>
                    </div>
                  )}
                </div>
              );
            })}
          </Card>
        </>
      )}

      {!loading && retired.length > 0 && (
        <div className="mt">
          <Card title={`${t("wdTitle")} (${retired.length})`}>
            <div className="muted" style={{ fontSize: 12.5, marginBottom: 10 }}>{t("wdPrivacyHint")}</div>
            {retired.map(tw => (
              <div className="tweak" key={tw.id} data-focus-id={tw.id}>
                <div className="tweak-head">
                  <span className="tweak-name">{tw.name}</span>
                  <Switch
                    on
                    label={tw.name}
                    title={t("wdReset")}
                    busy={busy === tw.id}
                    disabled={!!busy || !admin || !tw.canRevert}
                    onChange={() => doRevert(tw)}
                  />
                </div>
                <div className="tweak-detail">
                  <p>{reason(tw.id, tw.retiredReason)}</p>
                  {!tw.canRevert && <p className="muted">{t("wdNoCapture")}</p>}
                </div>
              </div>
            ))}
          </Card>
        </div>
      )}

      {log.length > 0 && (
        <div className="mt">
          <Card title={t("log")}>
            <div className="mono muted" style={{ fontSize: 11, lineHeight: 1.8, maxHeight: 160, overflowY: "auto" }}>
              {log.map((l, i) => (
                <div key={i} style={{ color: l.includes("✔") ? "var(--green)" : l.includes("✘") ? "var(--red)" : undefined }}>{l}</div>
              ))}
            </div>
          </Card>
        </div>
      )}
    </>
  );
}
