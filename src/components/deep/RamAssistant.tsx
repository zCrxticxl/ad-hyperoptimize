import React, { useState } from "react";
import { useLang, TKey } from "../../i18n";

type Plan = {
  platform: string; ddr: string; configured: number; rated: number | null; xmpOff: boolean;
  target: number; fclk: number | null; steps: string[];
};

const KEY = "ramAssistant.done";
const loadDone = (): string[] => { try { return JSON.parse(localStorage.getItem(KEY) || "[]"); } catch { return []; } };

/** Step-by-step BIOS guide for memory tuning, with values from the
 * detected kit. The app cannot set timings; "Check again" reads back the
 * configured speed after the user changed the BIOS. */
export function RamAssistant({ plan, onRecheck }: { plan: Plan; onRecheck: () => void }) {
  const { t } = useLang();
  const [open, setOpen] = useState(false);
  const [done, setDone] = useState<string[]>(loadDone);
  const toggle = (s: string) => {
    const next = done.includes(s) ? done.filter((d) => d !== s) : [...done, s];
    setDone(next);
    try { localStorage.setItem(KEY, JSON.stringify(next)); } catch {}
  };
  const fill = (s: string) =>
    s.replace("{rated}", String(plan.rated ?? "?")).replace("{configured}", String(plan.configured))
      .replace("{target}", String(plan.target)).replace("{fclk}", String(plan.fclk ?? "")).replace("{ddr}", plan.ddr);

  return (
    <div style={{ marginTop: 8, padding: 10, border: "1px solid var(--border)", borderRadius: 8, background: "var(--bg2)" }}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center", gap: 8 }}>
        <div style={{ fontSize: 13 }}>
          <b>{t("ramTitle")}</b>{" "}
          <span className="muted" style={{ fontSize: 12 }}>
            {fill(t("ramNow"))}{plan.rated ? ` · ${fill(t("ramRated"))}` : ""}
          </span>
        </div>
        <button className="btn small ghost" onClick={() => setOpen(!open)} aria-expanded={open}>{open ? "▲" : t("ramOpen")}</button>
      </div>
      {open && (
        <>
          <div className="muted" style={{ fontSize: 12, margin: "6px 0" }}>{t("ramIntro")}</div>
          <ol style={{ margin: 0, paddingLeft: 20 }}>
            {plan.steps.map((s) => (
              <li key={s} style={{ fontSize: 13, margin: "6px 0", opacity: done.includes(s) ? 0.55 : 1 }}>
                <label style={{ display: "flex", gap: 8, alignItems: "flex-start", cursor: "pointer" }}>
                  <input type="checkbox" checked={done.includes(s)} onChange={() => toggle(s)} style={{ marginTop: 3 }} />
                  <span>
                    <b>{fill(t(`ramStep_${s}` as TKey))}</b>
                    <div className="muted" style={{ fontSize: 12 }}>{fill(t(`ramStepText_${s}` as TKey))}</div>
                  </span>
                </label>
              </li>
            ))}
          </ol>
          <button className="btn small" style={{ marginTop: 6 }} onClick={onRecheck}>↺ {t("ramRecheck")}</button>
        </>
      )}
    </div>
  );
}
