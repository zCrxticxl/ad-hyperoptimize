import React from "react";
import { useLang, TKey } from "../../i18n";

type Row = {
  module: string; dpcs: number; isrs: number; totalMs: number; maxUs: number;
  over500us: number; over1ms: number; rating: "ok" | "warn" | "bad"; advice: string;
};
const COLOR = { ok: "var(--green)", warn: "var(--orange)", bad: "var(--red)" } as const;

/** Per-driver DPC/ISR summary of a WPR trace, worst first, with advice. */
export function DriverLatencyTable({ analysis }: { analysis: any }) {
  const { t } = useLang();
  if (!analysis) return null;
  if (analysis.error) return <div style={{ color: "var(--orange)", fontSize: 12 }} className="mt">{analysis.error}</div>;
  const rows: Row[] = analysis.modules ?? [];
  const worst = rows.filter((r) => r.rating !== "ok");
  const adviceKeys = [...new Set(worst.map((r) => r.advice))];
  return (
    <div className="mt">
      <div style={{ fontSize: 13, marginBottom: 6 }}>
        {worst.length === 0 ? (
          <span style={{ color: "var(--green)" }}>✓ {t("dlNoSpikes")}</span>
        ) : (
          <span style={{ color: COLOR[worst.some((r) => r.rating === "bad") ? "bad" : "warn"] }}>
            {t("dlWorst").replace("{m}", worst[0].module).replace("{us}", String(worst[0].maxUs))}
          </span>
        )}
      </div>
      <table className="tbl" style={{ fontSize: 12 }}>
        <thead><tr><th>{t("dlDriver")}</th><th>DPC</th><th>ISR</th><th>{t("dlMax")}</th><th>≥ 0,5 ms</th><th>≥ 1 ms</th><th>{t("dlTotal")}</th></tr></thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.module}>
              <td style={{ color: COLOR[r.rating] }}>{r.module}</td>
              <td>{r.dpcs}</td><td>{r.isrs}</td>
              <td style={{ color: COLOR[r.rating] }}>{r.maxUs} µs</td>
              <td>{r.over500us}</td><td>{r.over1ms}</td><td>{r.totalMs} ms</td>
            </tr>
          ))}
        </tbody>
      </table>
      {adviceKeys.map((k) => (
        <div key={k} style={{ fontSize: 12, marginTop: 6 }}>● <b>{t(`dlCat_${k}` as TKey)}:</b> <span className="muted">{t(`dlAdvice_${k}` as TKey)}</span></div>
      ))}
      <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>
        {t("dlNote").replace("{n}", String(analysis.events)).replace("{u}", String(analysis.unattributed))}
      </div>
    </div>
  );
}
