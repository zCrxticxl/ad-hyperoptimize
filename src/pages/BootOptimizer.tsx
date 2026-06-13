import React, { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Card, Spinner, Badge } from "../components/ui";
import { useLang } from "../i18n";

type BootEvent = {
  TimeCreated: string; BootDurationMs: number;
  MainPathBootMs: number; BootPostBootMs: number; SystemDriveInitMs: number;
};
type BcdTweak = {
  id: string; name: string; description: string;
  impact: string; risk: "Low" | "Medium" | "High"; applied: boolean;
};
type ScanData = {
  bootEvents: BootEvent[]; lastBootMs: number;
  logWasDisabled: boolean; bcd: Record<string, string>; tweaks: BcdTweak[];
};

const fmtMs = (ms: number) => {
  if (ms < 0)    return "–";
  if (ms < 1000) return `${ms} ms`;
  return `${(ms / 1000).toFixed(1)} s`;
};

const RISK_CLS: Record<string, string> = { Low: "risk-Low", Medium: "risk-Medium", High: "risk-High" };

function BootChart({ events, logWasDisabled }: { events: BootEvent[]; logWasDisabled: boolean }) {
  const { t, lang } = useLang();
  const fmt = (iso: string) => {
    try { return new Date(iso).toLocaleString(lang === "de" ? "de-DE" : "en-US", { dateStyle: "short", timeStyle: "short" }); }
    catch { return iso; }
  };
  const rating = (ms: number) => {
    if (ms < 0)      return { label: t("ratingUnknown"), color: "var(--muted)" };
    if (ms < 15_000) return { label: t("ratingExcellent"), color: "var(--green)" };
    if (ms < 25_000) return { label: t("ratingGood"), color: "#22b8cf" };
    if (ms < 45_000) return { label: t("ratingMedium"), color: "var(--yellow)" };
    return               { label: t("ratingSlow"), color: "var(--red)" };
  };

  if (events.length === 0) return (
    <div style={{ padding: "24px 16px", textAlign: "center" }}>
      <div style={{ color: "var(--yellow)", fontWeight: 600, marginBottom: 8, fontSize: 14 }}>
        {t("bootNoData")}
      </div>
      <div className="muted" style={{ fontSize: 12 }}>
        {logWasDisabled ? t("bootLogDisabled") : t("bootLogEmpty")}
        {" "}{t("bootRestartHint")}
      </div>
    </div>
  );

  const ordered = [...events].reverse();
  const max     = Math.max(...ordered.map(e => e.BootDurationMs), 1);

  return (
    <div>
      <div style={{ display: "flex", alignItems: "flex-end", gap: 4, height: 100, padding: "0 4px" }}>
        {ordered.map((e, i) => {
          const h = Math.max((e.BootDurationMs / max) * 88, 4);
          const r = rating(e.BootDurationMs);
          const isLast = i === ordered.length - 1;
          return (
            <div key={e.TimeCreated} title={`${fmt(e.TimeCreated)}: ${fmtMs(e.BootDurationMs)}`}
              style={{ flex: 1, display: "flex", flexDirection: "column", alignItems: "center", gap: 4, cursor: "default" }}>
              <div style={{
                width: "100%", height: h, borderRadius: "3px 3px 0 0",
                background: r.color, opacity: isLast ? 1 : 0.6,
                border: isLast ? `2px solid ${r.color}` : "none",
                boxShadow: isLast ? `0 0 8px ${r.color}66` : "none",
                transition: "height .3s",
              }} />
            </div>
          );
        })}
      </div>

      <div style={{ display: "flex", justifyContent: "space-between", marginTop: 4, fontSize: 10, color: "var(--muted)", padding: "0 4px" }}>
        <span>{fmt(ordered[0]?.TimeCreated ?? "")}</span>
        <span style={{ color: "var(--text)", fontWeight: 600 }}>
          {t("bootLastLabel")} {fmtMs(ordered[ordered.length - 1]?.BootDurationMs ?? -1)}
        </span>
        <span>{fmt(ordered[ordered.length - 1]?.TimeCreated ?? "")}</span>
      </div>

      {ordered.length > 0 && (() => {
        const last = ordered[ordered.length - 1];
        const total = last.BootDurationMs || 1;
        const segs = [
          { label: t("bootPhaseSystemInit"), ms: last.SystemDriveInitMs, color: "#4dabf7" },
          { label: t("bootPhaseMain"),       ms: last.MainPathBootMs,    color: "var(--accent)" },
          { label: t("bootPhasePost"),       ms: last.BootPostBootMs,    color: "var(--yellow)" },
        ].filter(s => s.ms > 0);
        if (segs.length === 0) return null;
        return (
          <div style={{ marginTop: 16 }}>
            <div className="muted" style={{ fontSize: 11, marginBottom: 8 }}>{t("bootPhaseBreakdown")}</div>
            {segs.map(s => (
              <div key={s.label} style={{ marginBottom: 8 }}>
                <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, marginBottom: 3 }}>
                  <span>{s.label}</span>
                  <span style={{ color: s.color, fontWeight: 600 }}>{fmtMs(s.ms)}</span>
                </div>
                <div style={{ height: 5, background: "var(--border)", borderRadius: 3 }}>
                  <div style={{ height: 5, borderRadius: 3, width: `${Math.min(s.ms / total * 100, 100)}%`, background: s.color }} />
                </div>
              </div>
            ))}
          </div>
        );
      })()}

      <div style={{ marginTop: 16 }}>
        <div className="muted" style={{ fontSize: 11, marginBottom: 6 }}>
          {t("bootHistoryLabel")} ({events.length} {t("bootEntries")})
        </div>
        <div style={{ overflowY: "auto", maxHeight: 180 }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12 }}>
            <thead>
              <tr style={{ color: "var(--muted)", borderBottom: "1px solid var(--border)" }}>
                <th style={{ textAlign: "left",  padding: "3px 8px" }}>{t("bootTblDate")}</th>
                <th style={{ textAlign: "right", padding: "3px 8px" }}>{t("bootTblTotal")}</th>
                <th style={{ textAlign: "right", padding: "3px 8px" }}>{t("bootTblMain")}</th>
                <th style={{ textAlign: "right", padding: "3px 8px" }}>{t("bootTblPost")}</th>
                <th style={{ textAlign: "left",  padding: "3px 8px" }}>{t("bootTblRating")}</th>
              </tr>
            </thead>
            <tbody>
              {[...events].map((e, i) => {
                const r = rating(e.BootDurationMs);
                return (
                  <tr key={e.TimeCreated} style={{ borderBottom: "1px solid var(--border)", fontWeight: i === 0 ? 600 : undefined }}>
                    <td style={{ padding: "4px 8px" }}>{fmt(e.TimeCreated)}</td>
                    <td style={{ padding: "4px 8px", textAlign: "right" }}>{fmtMs(e.BootDurationMs)}</td>
                    <td style={{ padding: "4px 8px", textAlign: "right", color: "var(--muted)" }}>{fmtMs(e.MainPathBootMs)}</td>
                    <td style={{ padding: "4px 8px", textAlign: "right", color: "var(--muted)" }}>{fmtMs(e.BootPostBootMs)}</td>
                    <td style={{ padding: "4px 8px" }}><span style={{ color: r.color, fontSize: 11 }}>● {r.label}</span></td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}

function BcdPanel({ bcd }: { bcd: Record<string, string> }) {
  const { t } = useLang();
  const interesting: [string, keyof ReturnType<typeof useLang>["t"] extends (k: infer K) => string ? K : never][] = [
    ["timeout",        "bcdTimeout"],
    ["bootmenupolicy", "bcdPolicy"],
    ["bootlog",        "bcdBootlog"],
    ["quietboot",      "bcdQuiet"],
    ["nx",             "bcdDep"],
    ["numproc",        "bcdNumproc"],
    ["testsigning",    "bcdTestsigning"],
    ["safeboot",       "bcdSafeboot"],
  ] as any;
  const rows = interesting.filter(([k]) => bcd[k] !== undefined);
  if (rows.length === 0) return <div className="muted" style={{ fontSize: 12 }}>{t("bootBcdUnreadable")}</div>;
  return (
    <div style={{ display: "flex", flexWrap: "wrap", gap: 10 }}>
      {rows.map(([k, lk]) => (
        <div key={k} style={{
          background: "var(--card2)", border: "1px solid var(--border)",
          borderRadius: 8, padding: "8px 14px", minWidth: 130,
        }}>
          <div className="muted" style={{ fontSize: 10, marginBottom: 2 }}>{t(lk as any)}</div>
          <div style={{ fontWeight: 700, fontFamily: "monospace", fontSize: 13 }}>{bcd[k]}</div>
        </div>
      ))}
    </div>
  );
}

function TweakRow({ tweak, admin, busy, onApply, onRevert }: {
  tweak: BcdTweak; admin: boolean; busy: boolean;
  onApply: () => void; onRevert: () => void;
}) {
  const { t } = useLang();
  const [open, setOpen] = useState(false);
  const needsAdmin = tweak.risk !== "Low";
  return (
    <div className="tweak">
      <div className="tweak-head">
        <span className="tweak-name" style={{ color: tweak.applied ? "var(--green)" : undefined }}>{tweak.name}</span>
        <Badge cls={RISK_CLS[tweak.risk]}>{tweak.risk}</Badge>
        <Badge cls={tweak.applied ? "st-applied" : "st-unknown"}>
          {tweak.applied ? t("active") : t("inactive")}
        </Badge>
        <button className="btn small ghost" onClick={() => setOpen(o => !o)}>{open ? "▲" : "▼"}</button>
        {tweak.applied ? (
          <button className="btn small ghost" disabled={busy || (!admin && needsAdmin)} onClick={onRevert}
            title={!admin && needsAdmin ? t("bootAdminNeeded") : ""}>
            {busy ? <Spinner /> : t("bootUndo")}
          </button>
        ) : (
          <button className="btn small" disabled={busy || (!admin && needsAdmin)} onClick={onApply}
            title={!admin && needsAdmin ? t("bootAdminNeeded") : ""}>
            {busy ? <Spinner /> : t("bootApply")}
          </button>
        )}
      </div>
      {open && (
        <div className="tweak-detail">
          <p>{tweak.description}</p>
          <p><b>{t("bootImpact")}:</b> {tweak.impact}</p>
          {!admin && needsAdmin && (
            <p style={{ color: "var(--yellow)" }}>⚠ {t("bootAdminNeeded")} (Risk: {tweak.risk})</p>
          )}
        </div>
      )}
    </div>
  );
}

export default function BootOptimizer({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData]       = useState<ScanData | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy]       = useState<string | null>(null);
  const [log, setLog]         = useState<string[]>([]);
  const [err, setErr]         = useState("");

  const push = (m: string) => setLog(l => [`[${new Date().toLocaleTimeString()}] ${m}`, ...l.slice(0, 49)]);

  const refresh = () => {
    setLoading(true); setErr("");
    api.bootScan().then(setData).catch(e => setErr(String(e))).finally(() => setLoading(false));
  };
  useEffect(() => { refresh(); }, []);

  const rating = useMemo(() => {
    const ms = data?.lastBootMs ?? -1;
    if (ms < 0)      return { label: t("ratingUnknown"), color: "var(--muted)" };
    if (ms < 15_000) return { label: t("ratingExcellent"), color: "var(--green)" };
    if (ms < 25_000) return { label: t("ratingGood"), color: "#22b8cf" };
    if (ms < 45_000) return { label: t("ratingMedium"), color: "var(--yellow)" };
    return               { label: t("ratingSlow"), color: "var(--red)" };
  }, [data, t]);

  const doApply = async (id: string) => {
    setBusy(id); push(`Applying ${id}…`);
    try { await api.bootTweakApply(id); push(`✔ ${id} applied`); }
    catch (e: any) { push(`✘ ${id}: ${e}`); setErr(String(e)); }
    finally { setBusy(null); refresh(); }
  };
  const doRevert = async (id: string) => {
    setBusy(id); push(`Reverting ${id}…`);
    try { await api.bootTweakRevert(id); push(`↩ ${id} reverted`); }
    catch (e: any) { push(`✘ ${id}: ${e}`); setErr(String(e)); }
    finally { setBusy(null); refresh(); }
  };

  const appliedCount = data?.tweaks.filter(tw => tw.applied).length ?? 0;
  const totalCount   = data?.tweaks.length ?? 0;

  return (
    <>
      <div className="page-title">{t("bootTitle")}</div>
      <div className="page-sub">{t("bootSub")}</div>

      {loading && <><Spinner /> <span className="muted">{t("bootLoading")}</span></>}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}

      {data && (
        <>
          <div style={{ display: "flex", gap: 14, flexWrap: "wrap", marginBottom: 16 }}>
            <div className="stat-chip" style={{ borderColor: rating.color }}>
              <div className="chip-val" style={{ color: rating.color }}>{data.lastBootMs > 0 ? `${(data.lastBootMs/1000).toFixed(1)} s` : "–"}</div>
              <div className="chip-lbl">{t("bootLast")}</div>
            </div>
            <div className="stat-chip">
              <div className="chip-val" style={{ color: rating.color }}>{rating.label}</div>
              <div className="chip-lbl">{t("bootRating")}</div>
            </div>
            <div className="stat-chip">
              <div className="chip-val">{appliedCount}/{totalCount}</div>
              <div className="chip-lbl">{t("bootTweaksActive")}</div>
            </div>
            <button className="btn small ghost" style={{ alignSelf: "center" }} onClick={refresh} disabled={loading}>
              {loading ? <Spinner /> : "↻ Rescan"}
            </button>
          </div>

          <Card title={t("bootChartTitle")}>
            <BootChart events={data.bootEvents} logWasDisabled={data.logWasDisabled} />
          </Card>

          <div className="mt">
            <Card title={t("bootBcdTitle")}>
              <BcdPanel bcd={data.bcd} />
              {!admin && (
                <div style={{ color: "var(--yellow)", fontSize: 12, marginTop: 10 }}>{t("bootAdminHint")}</div>
              )}
            </Card>
          </div>

          <div className="mt">
            <Card title={t("bootTweaksTitle")}>
              <div className="muted" style={{ fontSize: 12, marginBottom: 12 }}>{t("bootTweaksNote")}</div>
              {data.tweaks.map(tw => (
                <TweakRow key={tw.id} tweak={tw} admin={admin} busy={busy === tw.id}
                  onApply={() => doApply(tw.id)} onRevert={() => doRevert(tw.id)} />
              ))}
            </Card>
          </div>

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
      )}
    </>
  );
}
