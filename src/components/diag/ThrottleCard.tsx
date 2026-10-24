import React, { useEffect, useRef, useState } from "react";
import { api } from "../../api";
import { Card, Spinner, Switch } from "../ui";
import { useLang, TKey } from "../../i18n";

type Reason = { id: string; severity: "info" | "warn" | "bad" };
type Snap = {
  cpu: { limitPct: number | null; perfPct: number | null; limited: boolean };
  gpu: null | {
    tempC: number | null; powerW: number | null; powerLimitW: number | null;
    clockMhz: number | null; maxClockMhz: number | null; utilPct: number | null; reasons: Reason[];
  };
};

const SEV_COLOR = { info: "var(--accent)", warn: "var(--orange)", bad: "var(--red)" } as const;
const fmt = (v: number | null | undefined, unit: string, digits = 0) =>
  v == null ? "–" : `${v.toFixed(digits)} ${unit}`;

/** Live check whether CPU or GPU run below what they could, and why. */
export function ThrottleCard() {
  const { t } = useLang();
  const [live, setLive] = useState(true);
  const [snap, setSnap] = useState<Snap | null>(null);
  const [err, setErr] = useState("");
  // Seconds seen per throttle kind since the card opened.
  const [seen, setSeen] = useState<Record<string, number>>({});
  const busy = useRef(false);

  useEffect(() => {
    if (!live) return;
    let alive = true;
    const tick = async () => {
      if (busy.current) return;
      busy.current = true;
      try {
        const s: Snap = await api.throttleSnapshot();
        if (!alive) return;
        setSnap(s);
        setErr("");
        const kinds = [...(s.cpu.limited ? ["cpu"] : []), ...(s.gpu?.reasons ?? []).map((r) => r.id)];
        if (kinds.length) setSeen((p) => { const n = { ...p }; kinds.forEach((k) => (n[k] = (n[k] ?? 0) + 2)); return n; });
      } catch (e: any) {
        if (alive) setErr(String(e));
      } finally {
        busy.current = false;
      }
    };
    tick();
    const id = window.setInterval(tick, 2000);
    return () => { alive = false; window.clearInterval(id); };
  }, [live]);

  const reasonText = (id: string) => t(`thrReason_${id}` as TKey);

  return (
    <Card title={t("thrTitle")}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
        <div className="muted" style={{ fontSize: 12, flex: 1 }}>{t("thrDesc")}</div>
        <span className="muted" style={{ fontSize: 12, marginRight: 6 }}>{t("thrLive")}</span>
        <Switch on={live} label={t("thrLive")} onChange={setLive} />
      </div>
      {err && <div style={{ color: "var(--red)", fontSize: 12 }}>{err}</div>}
      {!snap && !err && <Spinner />}
      {snap && (
        <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(240px, 1fr))", gap: 12 }}>
          <div className="stat-chip" style={{ alignItems: "flex-start" }}>
            <b>CPU</b>
            <div style={{ fontSize: 13, marginTop: 4, color: snap.cpu.limited ? "var(--orange)" : "var(--green)" }}>
              {snap.cpu.limitPct == null ? t("thrUnknown") : snap.cpu.limited ? t("thrCpuLimited") : t("thrCpuFree")}
            </div>
            <div className="muted" style={{ fontSize: 12 }}>
              {t("thrCpuLimit")}: {fmt(snap.cpu.limitPct, "%")} · {t("thrCpuPerf")}: {fmt(snap.cpu.perfPct, "%")}
            </div>
          </div>
          <div className="stat-chip" style={{ alignItems: "flex-start" }}>
            <b>GPU</b>
            {!snap.gpu ? (
              <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>{t("thrNoNvidia")}</div>
            ) : (
              <>
                <div style={{ fontSize: 13, marginTop: 4, color: snap.gpu.reasons.length ? SEV_COLOR[snap.gpu.reasons.some((r) => r.severity === "bad") ? "bad" : snap.gpu.reasons.some((r) => r.severity === "warn") ? "warn" : "info"] : "var(--green)" }}>
                  {snap.gpu.reasons.length ? snap.gpu.reasons.map((r) => reasonText(r.id)).join(", ") : t("thrGpuFree")}
                </div>
                <div className="muted" style={{ fontSize: 12 }}>
                  {fmt(snap.gpu.tempC, "°C")} · {fmt(snap.gpu.powerW, "W")} / {fmt(snap.gpu.powerLimitW, "W")} · {fmt(snap.gpu.clockMhz, "MHz")} / {fmt(snap.gpu.maxClockMhz, "MHz")} · {fmt(snap.gpu.utilPct, "%")}
                </div>
              </>
            )}
          </div>
        </div>
      )}
      {snap?.gpu?.reasons.map((r) => (
        <div key={r.id} style={{ fontSize: 12, marginTop: 8, color: SEV_COLOR[r.severity] }}>
          ● {reasonText(r.id)}: <span className="muted">{t(`thrHint_${r.id}` as TKey)}</span>
        </div>
      ))}
      {snap?.cpu.limited && (
        <div style={{ fontSize: 12, marginTop: 8, color: "var(--orange)" }}>● {t("thrCpuLimited")}: <span className="muted">{t("thrHint_cpu")}</span></div>
      )}
      {Object.keys(seen).length > 0 && (
        <div className="muted" style={{ fontSize: 12, marginTop: 10 }}>
          {t("thrSeen")}: {Object.entries(seen).map(([k, s]) => `${k === "cpu" ? t("thrCpuLimited") : reasonText(k)} ~${s} s`).join(" · ")}
        </div>
      )}
    </Card>
  );
}
