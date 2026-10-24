import React, { useEffect, useState } from "react";
import { api } from "../../api";
import { Card, Spinner } from "../ui";
import { useLang } from "../../i18n";

type Drive = { letter: string; media: string; bus: string; size: number; free: number };
type Game = { name: string; launcher: string; path: string; drive: string | null };

/** Where installed games live and what those drives are. */
export function StorageCard() {
  const { t } = useLang();
  const [data, setData] = useState<{ games: Game[]; drives: Drive[]; trimEnabled: boolean | null; error?: string } | null>(null);
  const load = () => { setData(null); api.gameStorageCheck().then(setData).catch((e) => setData({ games: [], drives: [], trimEnabled: null, error: String(e) })); };
  useEffect(load, []);

  const drive = (l: string | null) => data?.drives.find((d) => d.letter.toUpperCase() === l?.toUpperCase());
  const freePct = (d: Drive) => (d.size > 0 ? Math.round((d.free / d.size) * 100) : null);
  const issues = (d?: Drive) => {
    if (!d) return [];
    const out: string[] = [];
    if (d.media === "HDD") out.push(t("stoHdd"));
    if (d.bus === "USB") out.push(t("stoUsb"));
    const f = freePct(d);
    if (f != null && f < 10) out.push(t("stoFull").replace("{p}", String(f)));
    return out;
  };
  const used = new Set((data?.games ?? []).map((g) => g.drive?.toUpperCase()).filter(Boolean));

  return (
    <Card title={t("stoTitle")}>
      <div className="row" style={{ justifyContent: "space-between", marginBottom: 8 }}>
        <div className="muted" style={{ fontSize: 12 }}>{t("stoDesc")}</div>
        <button className="btn small ghost" onClick={load}>↺</button>
      </div>
      {!data && <Spinner />}
      {data?.error && <div style={{ color: "var(--red)", fontSize: 12 }}>{data.error}</div>}
      {data && !data.error && (
        <>
          {data.drives.filter((d) => used.has(d.letter.toUpperCase())).map((d) => {
            const f = freePct(d);
            const bad = issues(d);
            return (
              <div key={d.letter} style={{ padding: "6px 0", borderBottom: "1px solid var(--border)" }}>
                <div className="row" style={{ gap: 10, flexWrap: "wrap" }}>
                  <b>{d.letter}:</b>
                  <span style={{ fontSize: 12 }}>{d.media || "?"} · {d.bus || "?"}</span>
                  <span className="muted" style={{ fontSize: 12 }}>{f != null ? `${f} % ${t("stoFree")}` : ""}</span>
                  {bad.length > 0 && <span style={{ fontSize: 12, color: "var(--orange)" }}>{bad.join(" · ")}</span>}
                </div>
                <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>
                  {data.games.filter((g) => g.drive?.toUpperCase() === d.letter.toUpperCase()).map((g) => g.name).join(", ")}
                </div>
              </div>
            );
          })}
          {data.games.length === 0 && <div className="muted">{t("stoNoGames")}</div>}
          {data.drives.some((d) => used.has(d.letter.toUpperCase()) && d.media === "HDD") && (
            <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>● {t("stoHddHint")}</div>
          )}
          <div style={{ fontSize: 12, marginTop: 8, color: data.trimEnabled === false ? "var(--orange)" : undefined }}>
            TRIM: {data.trimEnabled == null ? t("stoUnknown") : data.trimEnabled ? t("stoTrimOn") : t("stoTrimOff")}
          </div>
          <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>{t("stoSources")}</div>
        </>
      )}
    </Card>
  );
}
