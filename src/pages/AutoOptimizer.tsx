import React, { useEffect, useState, useMemo } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";

type Rec = {
  id: string; module: string; category: string; name: string;
  description: string; impact: string; risk: string; applied: boolean;
};

export default function AutoOptimizer({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData]       = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [busy, setBusy]       = useState(false);
  const [results, setResults] = useState<any>(null);

  const load = async (clearResults = false) => {
    setLoading(true);
    if (clearResults) setResults(null);
    try { setData(await api.autooptScan()); }
    finally { setLoading(false); }
  };

  useEffect(() => { load(true); }, []);

  const recs: Rec[] = data?.recs ?? [];
  const pending     = recs.filter(r => !r.applied);
  const applied     = recs.filter(r => r.applied);

  // Auto-select all pending on first load
  useEffect(() => {
    if (pending.length && selected.size === 0) {
      setSelected(new Set(pending.map(r => r.id)));
    }
  }, [data]);

  const grouped = useMemo(() => {
    const m: Record<string, Rec[]> = {};
    for (const r of pending) {
      if (!m[r.category]) m[r.category] = [];
      m[r.category].push(r);
    }
    return m;
  }, [pending]);

  const toggle = (id: string) => setSelected(prev => {
    const n = new Set(prev);
    if (n.has(id)) n.delete(id); else n.add(id);
    return n;
  });

  const applySelected = async () => {
    const items = pending.filter(r => selected.has(r.id));
    if (!items.length) return;
    setBusy(true);
    try {
      const res = await api.autooptApply(items);
      setResults(res);
      await load(false); // keep results visible after rescan
    } catch (e: any) {
      setResults({ error: String(e) });
    } finally { setBusy(false); }
  };

  // Score ring
  const score = data ? Math.round(((data.total - data.pending) / Math.max(data.total, 1)) * 100) : 0;
  const color = score >= 80 ? "var(--green)" : score >= 50 ? "var(--orange)" : "var(--red)";

  return (
    <>
      <div className="page-title">✨ {t("autoTitle")}</div>
      <div className="page-sub">
        {t("autoSub")}
        {!admin && <span style={{ color: "var(--orange)" }}> · {t("autoAdminHint")}</span>}
      </div>

      {loading ? (
        <Card title=""><div className="row" style={{ gap: 10 }}><Spinner /><span className="muted">{t("autoScanning")}</span></div></Card>
      ) : (
        <>
          {/* Score + summary */}
          <Card title="" style={{ marginBottom: 14 }}>
            <div className="row" style={{ gap: 24, alignItems: "center", flexWrap: "wrap" }}>
              <div style={{ position: "relative", width: 90, height: 90, flexShrink: 0 }}>
                <svg viewBox="0 0 36 36" style={{ width: 90, height: 90, transform: "rotate(-90deg)" }}>
                  <circle cx="18" cy="18" r="15.9" fill="none" stroke="var(--border)" strokeWidth="3" />
                  <circle cx="18" cy="18" r="15.9" fill="none" stroke={color} strokeWidth="3"
                    strokeDasharray={`${score} ${100 - score}`} strokeLinecap="round" />
                </svg>
                <div style={{ position: "absolute", inset: 0, display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center" }}>
                  <div style={{ fontSize: 22, fontWeight: 700, color }}>{score}</div>
                  <div style={{ fontSize: 10, color: "var(--muted)" }}>/ 100</div>
                </div>
              </div>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 15, fontWeight: 700, marginBottom: 4 }}>{t("autoScoreTitle")}</div>
                <div className="muted" style={{ fontSize: 13 }}>
                  {data.total - data.pending} {t("autoOf")} {data.total} {t("autoSafeTweaksApplied")} · {data.pending} {t("autoPendingWord")}
                </div>
                {data.pending === 0 && (
                  <div style={{ color: "var(--green)", fontWeight: 600, marginTop: 6, fontSize: 13 }}>
                    ✓ {t("autoFullyOptimized")}
                  </div>
                )}
              </div>
              <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
                {pending.length > 0 && (
                  <>
                    <button className="btn small ghost" onClick={() => setSelected(new Set(pending.map(r => r.id)))}>{t("autoSelectAll")}</button>
                    <button className="btn small ghost" onClick={() => setSelected(new Set())}>{t("autoClear")}</button>
                    <button className="btn" disabled={!selected.size || busy} onClick={applySelected}>
                      {busy ? <><Spinner /> {t("autoApplying")}</> : `⚡ ${t("autoApplyBtn")} (${selected.size})`}
                    </button>
                  </>
                )}
                <button className="btn small ghost" onClick={() => load(true)} disabled={loading || busy}>↺ {t("autoRescan")}</button>
              </div>
            </div>
          </Card>

          {/* Results */}
          {results && (
            <Card title={`${t("autoResultsTitle")} — ${results.ok ?? 0} ${t("autoOk")} · ${results.errors ?? 0} ${t("autoErrors")}`} style={{ marginBottom: 14 }}>
              {results.restore_point && (
                <div className="muted" style={{ fontSize: 11, marginBottom: 8 }}>🛟 {results.restore_point}</div>
              )}
              {results.error && (
                <div style={{ color: "var(--red)", fontSize: 12 }}>{results.error}</div>
              )}
              {(results.results ?? []).map((r: any) => (
                <div key={r.id} className="row" style={{ gap: 8, padding: "4px 0", borderBottom: "1px solid var(--border)", fontSize: 13 }}>
                  <span style={{ color: r.ok ? "var(--green)" : "var(--red)", fontWeight: 700, minWidth: 16 }}>
                    {r.ok ? "✓" : "✗"}
                  </span>
                  <span style={{ flex: 1 }}>{r.name}</span>
                  {!r.ok && <span className="muted" style={{ fontSize: 11 }}>{r.msg}</span>}
                </div>
              ))}
            </Card>
          )}

          {/* Pending tweaks */}
          {pending.length > 0 && (
            <Card title={`${t("autoPendingTitle")} (${pending.length})`} style={{ marginBottom: 14 }}>
              {Object.entries(grouped).map(([cat, items]) => (
                <div key={cat} style={{ marginBottom: 14 }}>
                  <div style={{ fontSize: 10, fontWeight: 700, letterSpacing: "0.08em", textTransform: "uppercase", color: "var(--muted)", marginBottom: 6, opacity: 0.7 }}>
                    {cat}
                  </div>
                  {items.map(r => (
                    <label key={r.id} className="row" style={{ gap: 10, padding: "7px 0", cursor: "pointer", borderBottom: "1px solid var(--border)", alignItems: "flex-start" }}>
                      <input type="checkbox" checked={selected.has(r.id)} onChange={() => toggle(r.id)} style={{ marginTop: 2, flexShrink: 0 }} />
                      <div style={{ flex: 1 }}>
                        <div style={{ fontWeight: 600, fontSize: 13 }}>{r.name}</div>
                        <div className="muted" style={{ fontSize: 11, marginTop: 2 }}>{r.description}</div>
                        {r.impact && <div style={{ fontSize: 11, color: "var(--green)", marginTop: 2 }}>↑ {r.impact}</div>}
                      </div>
                      <span style={{ fontSize: 10, padding: "2px 7px", borderRadius: 4, background: "var(--bg2)", color: "var(--muted)", flexShrink: 0, alignSelf: "center" }}>
                        {r.module === "tweak" ? t("autoBadgeTweak") : r.module === "privacy" ? t("autoBadgePrivacy") : t("autoBadgeDebloat")}
                      </span>
                    </label>
                  ))}
                </div>
              ))}
            </Card>
          )}

          {/* Already applied */}
          {applied.length > 0 && (
            <Card title={`${t("autoAppliedTitle")} (${applied.length})`}>
              <div style={{ maxHeight: 200, overflowY: "auto" }}>
                {applied.map(r => (
                  <div key={r.id} className="row" style={{ gap: 8, padding: "5px 0", borderBottom: "1px solid var(--border)", fontSize: 12 }}>
                    <span style={{ color: "var(--green)" }}>✓</span>
                    <span style={{ flex: 1 }}>{r.name}</span>
                    <span className="muted" style={{ fontSize: 11 }}>{r.category}</span>
                  </div>
                ))}
              </div>
            </Card>
          )}
        </>
      )}
    </>
  );
}
