import React, { useRef, useState } from "react";
import { Card } from "../ui";
import { useLang } from "../../i18n";
import { useHealthScan } from "../../hooks/useHealthScan";

const STANDARD = [125, 250, 500, 1000, 2000, 4000, 8000];
const nearest = (hz: number) => STANDARD.reduce((a, b) => (Math.abs(b - hz) < Math.abs(a - hz) ? b : a));

/** Mouse polling rate from raw pointer events, plus the input settings the
 * health scan already reads. */
export function InputCard() {
  const { t } = useLang();
  const { scan } = useHealthScan();
  const [state, setState] = useState<"idle" | "measuring" | "done">("idle");
  const [hz, setHz] = useState<number | null>(null);
  const stamps = useRef<number[]>([]);
  const timer = useRef<number | null>(null);

  const onMove = (e: React.PointerEvent | PointerEvent) => {
    if (state !== "measuring") return;
    const ev = ("nativeEvent" in e ? e.nativeEvent : e) as PointerEvent;
    const list = typeof ev.getCoalescedEvents === "function" ? ev.getCoalescedEvents() : [];
    if (list.length) list.forEach((c) => stamps.current.push(c.timeStamp));
    else stamps.current.push(ev.timeStamp);
  };

  const start = () => {
    stamps.current = [];
    setHz(null);
    setState("measuring");
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      const ts = [...new Set(stamps.current)].sort((a, b) => a - b);
      // Intervals while the mouse moved; pauses (> 50 ms) are left out.
      const iv = ts.slice(1).map((v, i) => v - ts[i]).filter((d) => d > 0 && d < 50).sort((a, b) => a - b);
      setHz(iv.length >= 50 ? 1000 / iv[Math.floor(iv.length / 2)] : null);
      setState("done");
    }, 3000);
  };

  const item = (id: string) => scan?.items.find((i) => i.id === id);
  const accel = item("mouse_accel_off");
  const usb = item("usb_suspend_off");
  const statusText = (s?: string) => (s === "applied" ? t("inpOff") : s === "not_applied" ? t("inpOn") : "–");

  return (
    <Card title={t("inpTitle")}>
      <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>{t("inpDesc")}</div>
      <div
        onPointerMove={onMove}
        style={{
          height: 120, borderRadius: 8, border: `1px dashed ${state === "measuring" ? "var(--green)" : "var(--border)"}`,
          display: "grid", placeItems: "center", userSelect: "none", cursor: "crosshair", background: "var(--bg2)",
        }}
      >
        {state === "measuring" ? (
          <span style={{ color: "var(--green)" }}>{t("inpMoving")}</span>
        ) : (
          <button className="btn small" onClick={start}>{t("inpStart")}</button>
        )}
      </div>
      {state === "done" && (
        <div style={{ marginTop: 8, fontSize: 13 }}>
          {hz == null ? (
            <span style={{ color: "var(--orange)" }}>{t("inpTooFew")}</span>
          ) : (
            <>
              {t("inpResult")}: <b>≈ {nearest(hz)} Hz</b> <span className="muted">({Math.round(hz)} Hz {t("inpMeasured")})</span>
              {nearest(hz) < 1000 && <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>{t("inpLowHint")}</div>}
            </>
          )}
        </div>
      )}
      <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>{t("inpNote")}</div>
      <div style={{ fontSize: 12, marginTop: 10 }}>
        {t("inpAccel")}: <b>{statusText(accel?.status)}</b> · {t("inpUsb")}: <b>{statusText(usb?.status)}</b>
        <div className="muted" style={{ fontSize: 11 }}>{t("inpChangeIn")}</div>
      </div>
    </Card>
  );
}
