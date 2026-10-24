import React, { useEffect, useState } from "react";
import { api } from "../../api";
import { Card, Spinner } from "../ui";
import { useLang, TKey } from "../../i18n";

type Mon = {
  name: string; connection: string; width: number; height: number; hz: number; maxHz: number;
  hdrSupported: boolean; hdrOn: boolean; bitsPerChannel: number; hints: string[];
};

/** Per monitor: connection, refresh rate vs. the highest offered, HDR, bit depth. */
export function DisplayCard() {
  const { t } = useLang();
  const [data, setData] = useState<{ monitors: Mon[]; vrrWindowed: boolean | null; error?: string } | null>(null);
  const load = () => { setData(null); api.displayCheck().then(setData).catch((e) => setData({ monitors: [], vrrWindowed: null, error: String(e) })); };
  useEffect(load, []);
  const conn = (c: string) => (c === "internal" || c === "other" || c === "indirect" ? t(`dispConn_${c}` as TKey) : c);

  return (
    <Card title={t("dispTitle")}>
      <div className="row" style={{ justifyContent: "space-between", marginBottom: 8 }}>
        <div className="muted" style={{ fontSize: 12 }}>{t("dispDesc")}</div>
        <button className="btn small ghost" onClick={load}>↺</button>
      </div>
      {!data && <Spinner />}
      {data?.error && <div style={{ color: "var(--red)", fontSize: 12 }}>{data.error}</div>}
      {data?.monitors.map((m, i) => (
        <div key={i} style={{ padding: "8px 0", borderBottom: "1px solid var(--border)" }}>
          <div className="row" style={{ gap: 10, flexWrap: "wrap", alignItems: "baseline" }}>
            <b style={{ fontSize: 13 }}>{m.name}</b>
            <span className="muted" style={{ fontSize: 12 }}>{conn(m.connection)}</span>
            <span style={{ fontSize: 12, color: m.hints.includes("hzBelowMax") ? "var(--orange)" : "var(--green)" }}>
              {m.width}×{m.height} @ {Math.round(m.hz)} Hz{m.maxHz > Math.round(m.hz) + 1 ? ` (${t("dispMax")} ${m.maxHz} Hz)` : ""}
            </span>
            <span className="muted" style={{ fontSize: 12 }}>
              HDR: {m.hdrOn ? t("dispOn") : m.hdrSupported ? t("dispOffSupported") : t("dispUnsupported")}
              {m.bitsPerChannel > 0 ? ` · ${m.bitsPerChannel} Bit` : ""}
            </span>
          </div>
          {m.hints.map((h) => (
            <div key={h} style={{ fontSize: 12, marginTop: 4, color: h === "hzBelowMax" ? "var(--orange)" : "var(--muted)" }}>
              ● {t(`dispHint_${h}` as TKey).replace("{max}", String(m.maxHz))}
            </div>
          ))}
        </div>
      ))}
      {data && data.vrrWindowed != null && (
        <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>
          {t("dispVrr")}: {data.vrrWindowed ? t("dispOn") : t("dispOff")}
        </div>
      )}
      {data && !data.error && <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>{t("dispGsyncNote")}</div>}
    </Card>
  );
}
