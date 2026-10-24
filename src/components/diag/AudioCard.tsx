import React, { useEffect, useState } from "react";
import { api } from "../../api";
import { Card, Spinner } from "../ui";
import { useLang, TKey } from "../../i18n";

type Dev = { name: string; interface: string; sampleRate: number | null; bits: number | null; channels: number | null; hints: string[] };

/** Active playback devices with the format Windows mixes to. */
export function AudioCard() {
  const { t } = useLang();
  const [data, setData] = useState<{ devices: Dev[]; error?: string } | null>(null);
  const load = () => { setData(null); api.audioCheck().then(setData).catch((e) => setData({ devices: [], error: String(e) })); };
  useEffect(load, []);

  return (
    <Card title={t("audTitle")}>
      <div className="row" style={{ justifyContent: "space-between", marginBottom: 8 }}>
        <div className="muted" style={{ fontSize: 12 }}>{t("audDesc")}</div>
        <button className="btn small ghost" onClick={load}>↺</button>
      </div>
      {!data && <Spinner />}
      {data?.error && <div style={{ color: "var(--red)", fontSize: 12 }}>{data.error}</div>}
      {data && !data.error && data.devices.length === 0 && <div className="muted">{t("audNone")}</div>}
      {data?.devices.map((d, i) => (
        <div key={i} style={{ padding: "8px 0", borderBottom: "1px solid var(--border)" }}>
          <div className="row" style={{ gap: 10, flexWrap: "wrap", alignItems: "baseline" }}>
            <b style={{ fontSize: 13 }}>{d.name || "–"}</b>
            <span className="muted" style={{ fontSize: 12 }}>{d.interface}</span>
            <span style={{ fontSize: 12 }}>
              {d.sampleRate ? `${d.sampleRate / 1000} kHz` : "–"}{d.bits ? ` · ${d.bits} Bit` : ""}{d.channels ? ` · ${d.channels} ${t("audChannels")}` : ""}
            </span>
          </div>
          {d.hints.map((h) => (
            <div key={h} className="muted" style={{ fontSize: 12, marginTop: 4 }}>● {t(`audHint_${h}` as TKey)}</div>
          ))}
        </div>
      ))}
      {data && !data.error && <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>{t("audWhere")}</div>}
    </Card>
  );
}
