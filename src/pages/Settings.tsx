import React, { useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang, LANG_NAMES, Lang } from "../i18n";
import type { Mode } from "../App";

export default function Settings({
  mode,
  setMode,
}: {
  mode: Mode;
  setMode: (m: Mode) => void;
}) {
  const { t, lang, setLang } = useLang();
  const [clearing, setClearing] = useState(false);
  const [cacheMsg, setCacheMsg] = useState("");
  const [err, setErr] = useState("");

  const clearCache = async () => {
    setClearing(true);
    setCacheMsg("");
    setErr("");
    try {
      setCacheMsg(await api.clearCache());
    } catch (e: any) {
      setErr(String(e));
    } finally {
      setClearing(false);
    }
  };

  return (
    <>
      <h1 className="page-title">⚙️ {t("settingsTitle")}</h1>
      <div className="page-sub">{t("settingsSub")}</div>

      <Card title={t("settingsAppearance")}>
        <div style={{ display: "flex", flexDirection: "column", gap: 14 }}>
          {/* Mode */}
          <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
            <div>
              <div style={{ fontWeight: 600, fontSize: 13 }}>{t("settingsMode")}</div>
              <div className="muted" style={{ fontSize: 12 }}>{t("settingsModeHint")}</div>
            </div>
            <div className="mode-toggle" style={{ width: 200 }}>
              <button className={mode === "beginner" ? "on" : ""} onClick={() => setMode("beginner")}>{t("beginner")}</button>
              <button className={mode === "expert" ? "on" : ""} onClick={() => setMode("expert")}>{t("expert")}</button>
            </div>
          </div>
          {/* Language */}
          <div className="row" style={{ justifyContent: "space-between", alignItems: "center" }}>
            <div>
              <div style={{ fontWeight: 600, fontSize: 13 }}>{t("settingsLanguage")}</div>
              <div className="muted" style={{ fontSize: 12 }}>{t("settingsLanguageHint")}</div>
            </div>
            <select
              className="select"
              style={{ minWidth: 180 }}
              value={lang}
              onChange={(e) => setLang(e.target.value as Lang)}
              aria-label={t("settingsLanguage")}
            >
              {Object.entries(LANG_NAMES).map(([code, name]) => (
                <option key={code} value={code}>{name}</option>
              ))}
            </select>
          </div>
        </div>
      </Card>

      <Card title={t("settingsData")} style={{ marginTop: 14 }}>
        <div className="row" style={{ justifyContent: "space-between", alignItems: "center", flexWrap: "wrap" }}>
          <div style={{ flex: 1, minWidth: 220 }}>
            <div style={{ fontWeight: 600, fontSize: 13 }}>{t("settingsCache")}</div>
            <div className="muted" style={{ fontSize: 12 }}>{t("settingsCacheHint")}</div>
          </div>
          <button className="btn ghost" disabled={clearing} onClick={clearCache}>
            {clearing ? <><Spinner /> {t("settingsClearing")}</> : t("settingsClearCache")}
          </button>
        </div>
        {cacheMsg && <div style={{ marginTop: 10, color: "var(--green)", fontSize: 12 }}>{cacheMsg}</div>}
        {err && <div style={{ marginTop: 10, color: "var(--red)", fontSize: 12 }}>{err}</div>}
      </Card>

      <Card title={t("settingsAbout")} style={{ marginTop: 14 }}>
        <div className="row" style={{ gap: 20 }}>
          <div>
            <div className="muted" style={{ fontSize: 12 }}>{t("settingsVersion")}</div>
            <div style={{ fontWeight: 700 }}>v1.4.0</div>
          </div>
          <div>
            <div className="muted" style={{ fontSize: 12 }}>{t("settingsMode")}</div>
            <div style={{ fontWeight: 700 }}>{mode === "expert" ? t("expert") : t("beginner")}</div>
          </div>
        </div>
        <div className="muted" style={{ fontSize: 12, marginTop: 10, lineHeight: 1.5 }}>
          {t("settingsDataNote")}
        </div>
      </Card>
    </>
  );
}
