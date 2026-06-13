import React, { useRef, useState } from "react";
import { api } from "../api";
import { Card, Spinner, Badge } from "../components/ui";
import { useLang } from "../i18n";

type CheckResult = {
  kind: string; label: string; output: string;
  result: "clean" | "repaired" | "corrupt" | "error" | "unknown";
};

const RESULT_META: Record<string, { cls: string; icon: string; labelKey: string }> = {
  clean:    { cls: "st-applied",  icon: "✔", labelKey: "clean" },
  repaired: { cls: "risk-Medium", icon: "⚡", labelKey: "repaired" },
  corrupt:  { cls: "risk-High",   icon: "✘", labelKey: "corrupt" },
  error:    { cls: "risk-High",   icon: "✘", labelKey: "error" },
  unknown:  { cls: "st-unknown",  icon: "?", labelKey: "unknown" },
};

type CheckDef = {
  id: string; labelKey: string; icon: string;
  descKey: string; timeKey: string;
};

const CHECKS: CheckDef[] = [
  { id: "sfc",            icon: "🔧", labelKey: "sfcLabel",            descKey: "sfcDesc",            timeKey: "sfcTime" },
  { id: "dism_check",     icon: "🩺", labelKey: "dismCheckLabel",      descKey: "dismCheckDesc",      timeKey: "dismCheckTime" },
  { id: "dism_scan",      icon: "🔍", labelKey: "dismScanLabel",       descKey: "dismScanDesc",       timeKey: "dismScanTime" },
  { id: "dism_restore",   icon: "♻", labelKey: "dismRestoreLabel",    descKey: "dismRestoreDesc",    timeKey: "dismRestoreTime" },
  { id: "dism_component", icon: "🧹", labelKey: "dismComponentLabel",  descKey: "dismComponentDesc",  timeKey: "dismComponentTime" },
];

// Static labels that don't change with lang (they're the actual bcdedit command names)
const CHECK_LABELS: Record<string, string> = {
  sfc:            "SFC /scannow",
  dism_check:     "DISM CheckHealth",
  dism_scan:      "DISM ScanHealth",
  dism_restore:   "DISM RestoreHealth",
  dism_component: "DISM ComponentCleanup",
};

export default function HealthCheck({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [results, setResults] = useState<Record<string, CheckResult>>({});
  const [running, setRunning] = useState<string | null>(null);
  const [err, setErr]         = useState("");
  const logRef                = useRef<HTMLDivElement>(null);
  const [log, setLog]         = useState<string[]>([]);

  const push = (m: string) => {
    setLog(l => {
      const next = [...l, `[${new Date().toLocaleTimeString()}] ${m}`];
      setTimeout(() => logRef.current?.scrollTo({ top: 99999, behavior: "smooth" }), 50);
      return next;
    });
  };

  const run = async (id: string) => {
    if (!admin) { setErr(t("adminRequired")); return; }
    setRunning(id); setErr("");
    const label = CHECK_LABELS[id];
    push(`▶ ${label}…`);
    try {
      const r = await api.healthRun(id) as CheckResult;
      setResults(prev => ({ ...prev, [id]: r }));
      const meta = RESULT_META[r.result] ?? RESULT_META.unknown;
      push(`${meta.icon} ${label}: ${t(meta.labelKey as any)}`);
      if (r.output) r.output.split("\n").slice(0, 20).forEach(l => push(l));
    } catch (e: any) {
      setErr(String(e));
      push(`✘ ${label}: ${e}`);
    } finally { setRunning(null); }
  };

  const allDone   = Object.values(results).length;
  const hasCorrupt = Object.values(results).some(r => r.result === "corrupt");

  return (
    <>
      <div className="page-title">{t("healthTitle")}</div>
      <div className="page-sub">{t("healthSub")}</div>

      {!admin && <div className="warn-banner">{t("healthAdminWarn")}</div>}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}

      {allDone > 0 && (
        <div className="status-banner" style={{
          background: hasCorrupt ? "rgba(239,68,68,.1)" : "rgba(34,197,94,.1)",
          borderColor: hasCorrupt ? "var(--red)" : "var(--green)",
          marginBottom: 16,
        }}>
          {hasCorrupt
            ? t("healthCorrupt")
            : `✔ ${allDone} ${t("healthAllGood")}`}
        </div>
      )}

      <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
        {CHECKS.map(chk => {
          const res    = results[chk.id];
          const isRun  = running === chk.id;
          const meta   = res ? (RESULT_META[res.result] ?? RESULT_META.unknown) : null;
          const label  = CHECK_LABELS[chk.id];
          const desc   = t(chk.descKey as any);
          const time   = t(chk.timeKey as any);
          return (
            <Card key={chk.id} title="">
              <div style={{ display: "flex", alignItems: "flex-start", gap: 14 }}>
                <div style={{ fontSize: 28, lineHeight: 1, minWidth: 32 }}>{chk.icon}</div>
                <div style={{ flex: 1 }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 8, flexWrap: "wrap" }}>
                    <span style={{ fontWeight: 700 }}>{label}</span>
                    <span className="muted" style={{ fontSize: 11 }}>{time}</span>
                    {meta && <Badge cls={meta.cls}>{meta.icon} {t(meta.labelKey as any)}</Badge>}
                  </div>
                  <div className="muted" style={{ fontSize: 12, marginTop: 3 }}>{desc}</div>
                  {res && res.output && (
                    <details style={{ marginTop: 8 }}>
                      <summary className="muted" style={{ cursor: "pointer", fontSize: 11 }}>
                        {t("output")} ({res.output.split("\n").length} {t("lines")})
                      </summary>
                      <pre style={{
                        fontSize: 10, lineHeight: 1.6,
                        background: "var(--bg2)", borderRadius: 4,
                        padding: "6px 8px", margin: "6px 0 0",
                        maxHeight: 200, overflowY: "auto",
                        whiteSpace: "pre-wrap", wordBreak: "break-all",
                        color: "var(--muted)",
                      }}>
                        {res.output}
                      </pre>
                    </details>
                  )}
                </div>
                <button
                  className="btn"
                  disabled={!!running || !admin}
                  onClick={() => run(chk.id)}
                  style={{ minWidth: 100, flexShrink: 0 }}
                >
                  {isRun ? <><Spinner /> {t("running")}</> : t("run")}
                </button>
              </div>
            </Card>
          );
        })}
      </div>

      <div style={{ marginTop: 16 }}>
        <Card title={t("healthWorkflow")}>
          <div className="muted" style={{ fontSize: 13, lineHeight: 2 }}>
            <div>1. <strong>DISM CheckHealth</strong> — {t("hw1")}</div>
            <div>2. <strong>SFC /scannow</strong> — {t("hw2")}</div>
            <div>3. {t("hw3")}</div>
            <div>4. {t("hw4")}</div>
            <div>5. {t("hw5")}</div>
          </div>
        </Card>
      </div>

      {log.length > 0 && (
        <div style={{ marginTop: 16 }}>
          <Card title={t("liveLog")}>
            <div
              ref={logRef}
              className="mono"
              style={{
                fontSize: 11, lineHeight: 1.8,
                maxHeight: 220, overflowY: "auto",
                background: "var(--bg2)", borderRadius: 4,
                padding: "6px 8px",
              }}
            >
              {log.map((l, i) => {
                const col = l.includes("✔") ? "var(--green)"
                  : l.includes("✘") ? "var(--red)"
                  : l.includes("▶") ? "var(--accent)"
                  : "var(--muted)";
                return <div key={i} style={{ color: col }}>{l}</div>;
              })}
            </div>
          </Card>
        </div>
      )}
    </>
  );
}
