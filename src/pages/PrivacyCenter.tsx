import React, { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Card, Spinner, Badge } from "../components/ui";
import { useLang } from "../i18n";

type PTweak = {
  id: string; name: string; category: string;
  description: string; risk: "Low" | "Medium" | "High"; applied: boolean;
};

const RISK_CLS: Record<string, string> = { Low: "risk-Low", Medium: "risk-Medium", High: "risk-High" };

// Keys are backend category names (always German); icons and display labels are translated
const CAT_ICONS: Record<string, string> = {
  "Telemetrie": "📡",
  "Werbung": "📢",
  "Suche": "🔍",
  "Aktivitätsverlauf": "📋",
  "Eingabe": "⌨",
  "Standort": "📍",
  "Feedback": "💬",
  "Netzwerk": "🌐",
};

export default function PrivacyCenter({ admin }: { admin: boolean }) {
  const { t } = useLang();

  // Translate backend category names (backend always returns German keys)
  const CAT_LABEL: Record<string, string> = {
    "Telemetrie":       t("catTelemetrie"),
    "Werbung":          t("catWerbung"),
    "Suche":            t("catSuche"),
    "Aktivitätsverlauf":t("catAktivitaet"),
    "Eingabe":          t("catEingabe"),
    "Standort":         t("catStandort"),
    "Feedback":         t("catFeedback"),
    "Netzwerk":         t("catNetzwerk"),
  };
  const [tweaks, setTweaks]   = useState<PTweak[]>([]);
  const [applied, setApplied] = useState(0);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy]       = useState<string | null>(null);
  const [log, setLog]         = useState<string[]>([]);
  const [err, setErr]         = useState("");
  const [filter, setFilter]   = useState<string | null>(null);
  const [openId, setOpenId]   = useState<string | null>(null);

  const push = (m: string) =>
    setLog(l => [`[${new Date().toLocaleTimeString()}] ${m}`, ...l.slice(0, 49)]);

  const refresh = () => {
    setLoading(true);
    api.privacyScan()
      .then((r: any) => { setTweaks(r.tweaks ?? []); setApplied(r.applied ?? 0); })
      .catch(e => setErr(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => { refresh(); }, []);

  const categories = useMemo(() => [...new Set(tweaks.map(t => t.category))], [tweaks]);

  const visibleTweaks = useMemo(() =>
    filter ? tweaks.filter(t => t.category === filter) : tweaks,
    [tweaks, filter]);

  const doApply = async (tw: PTweak) => {
    setBusy(tw.id);
    push(`Applying ${tw.name}…`);
    try {
      await api.privacyApply(tw.id);
      push(`✔ ${tw.name} ${t("active")}`);
    } catch (e: any) { push(`✘ ${tw.name}: ${e}`); setErr(String(e)); }
    finally { setBusy(null); refresh(); }
  };

  const doRevert = async (tw: PTweak) => {
    setBusy(tw.id);
    push(`Reverting ${tw.name}…`);
    try {
      await api.privacyRevert(tw.id);
      push(`↩ ${tw.name} reverted`);
    } catch (e: any) { push(`✘ ${tw.name}: ${e}`); setErr(String(e)); }
    finally { setBusy(null); refresh(); }
  };

  const applyAll = async () => {
    const pending = tweaks.filter(t => !t.applied);
    for (const t of pending) await doApply(t);
  };

  const totalApplied = tweaks.filter(t => t.applied).length;
  const pct = tweaks.length > 0 ? Math.round(totalApplied / tweaks.length * 100) : 0;

  return (
    <>
      <div className="page-title">{t("privTitle")}</div>
      <div className="page-sub">{t("privSub")}</div>

      {loading && <><Spinner /> <span className="muted">{t("privLoading")}</span></>}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}

      {!loading && tweaks.length > 0 && (
        <>
          {/* Score card */}
          <Card title="">
            <div style={{ display: "flex", alignItems: "center", gap: 20, flexWrap: "wrap" }}>
              <div style={{ textAlign: "center", minWidth: 90 }}>
                <div style={{
                  fontSize: 36, fontWeight: 900,
                  color: pct >= 80 ? "var(--green)" : pct >= 50 ? "var(--yellow)" : "var(--red)"
                }}>{pct}%</div>
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
                <button className="btn" disabled={!!busy || totalApplied === tweaks.length} onClick={applyAll}>
                  {t("applyAll")}
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
                <div className="tweak" key={tw.id}>
                  <div className="tweak-head">
                    <span className="tweak-name" style={{ color: tw.applied ? "var(--green)" : undefined }}>
                      {tw.name}
                    </span>
                    <Badge cls={RISK_CLS[tw.risk]}>{tw.risk}</Badge>
                    <Badge cls={tw.applied ? "st-applied" : "st-unknown"}>
                      {tw.applied ? t("active") : t("inactive")}
                    </Badge>
                    <button className="btn small ghost" onClick={() => setOpenId(isOpen ? null : tw.id)}>
                      {isOpen ? "▲" : "▼"}
                    </button>
                    {tw.applied ? (
                      <button className="btn small ghost" disabled={isBusy} onClick={() => doRevert(tw)}>
                        {isBusy ? <Spinner /> : t("revert")}
                      </button>
                    ) : (
                      <button className="btn small" disabled={isBusy} onClick={() => doApply(tw)}>
                        {isBusy ? <Spinner /> : t("privActivate")}
                      </button>
                    )}
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

          {/* Log */}
          {log.length > 0 && (
            <div className="mt">
              <Card title={t("log")}>
                <div className="mono muted" style={{ fontSize: 11, lineHeight: 1.8, maxHeight: 160, overflowY: "auto" }}>
                  {log.map((l, i) => (
                    <div key={i} style={{ color: l.includes("✔") ? "var(--green)" : l.includes("✘") ? "var(--red)" : undefined }}>
                      {l}
                    </div>
                  ))}
                </div>
              </Card>
            </div>
          )}
        </>
      )}
    </>
  );
}
