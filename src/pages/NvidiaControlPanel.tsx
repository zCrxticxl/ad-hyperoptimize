import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";

type NvData = {
  supported: boolean;
  name?: string;
  driverKeyMissing?: boolean;
  powerManagementMode?: "adaptive" | "preferMaxPerformance";
  maxPreRenderedFrames?: string;
  lowLatencyMode?: "off" | "on" | "ultra";
  threadedOptimization?: "off" | "on";
};

type SettingKey = "powerManagementMode" | "maxPreRenderedFrames" | "lowLatencyMode" | "threadedOptimization";

export default function NvidiaControlPanel() {
  const { t } = useLang();
  const [data, setData] = useState<NvData | null>(null);
  const [loading, setLoading] = useState(true);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState<SettingKey | null>(null);
  const [log, setLog] = useState("");
  const [tokens, setTokens] = useState<Record<SettingKey, string | null>>({
    powerManagementMode: null,
    maxPreRenderedFrames: null,
    lowLatencyMode: null,
    threadedOptimization: null,
  });
  const [opening, setOpening] = useState(false);

  const refresh = () => {
    setLoading(true);
    setErr("");
    api.nvGetSettings()
      .then(setData)
      .catch((e: any) => setErr(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => { refresh(); }, []);

  const setSetting = async (setting: SettingKey, value: string) => {
    setBusy(setting);
    setLog("");
    try {
      const r = await api.nvSetSetting(setting, value);
      setTokens((prev) => ({ ...prev, [setting]: r.restoreToken }));
      setLog(t("nvcpApplied"));
      await refresh();
    } catch (e: any) {
      setLog(String(e));
    } finally {
      setBusy(null);
    }
  };

  const undoSetting = async (setting: SettingKey) => {
    const token = tokens[setting];
    if (!token) return;
    setBusy(setting);
    try {
      await api.revertEntry(token);
      setTokens((prev) => ({ ...prev, [setting]: null }));
      setLog(t("nvcpUndone"));
      await refresh();
    } catch (e: any) {
      setLog(String(e));
    } finally {
      setBusy(null);
    }
  };

  const openPanel = async () => {
    setOpening(true);
    try { await api.nvOpenPanel(); }
    catch (e: any) { setLog(String(e)); }
    finally { setOpening(false); }
  };

  const Row = ({
    setting, label, value, options,
  }: { setting: SettingKey; label: string; value: string | undefined; options: [string, string][] }) => (
    <div className="row" style={{ padding: "10px 0", borderBottom: "1px solid var(--border)", gap: 12, alignItems: "center" }}>
      <div style={{ flex: 1, fontWeight: 600, fontSize: 13 }}>{label}</div>
      <select
        value={value ?? options[0][0]}
        disabled={busy === setting}
        onChange={(e) => setSetting(setting, e.target.value)}
        style={{ minWidth: 200 }}
      >
        {options.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
      </select>
      {tokens[setting] && (
        <button className="btn small ghost" disabled={busy === setting} onClick={() => undoSetting(setting)}>
          {busy === setting ? <Spinner /> : t("revert")}
        </button>
      )}
    </div>
  );

  return (
    <>
      <div className="page-title">🟢 {t("nvcpTitle")}</div>
      <div className="page-sub">{t("nvcpSub")}</div>

      {loading && <><Spinner /> <span className="muted">{t("gpuLoading")}</span></>}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}

      {data && !data.supported && (
        <div style={{ color: "var(--orange)", fontSize: 13, marginTop: 12, padding: "10px 14px", background: "rgba(255,160,0,0.08)", borderRadius: 6, border: "1px solid var(--orange)" }}>
          ⚠ {t("nvcpNoGpu")}
        </div>
      )}

      {data && data.supported && (
        <Card title={`${t("nvcpGlobalSettings")} — ${data.name ?? "NVIDIA"}`}>
          {data.driverKeyMissing && (
            <div style={{ color: "var(--orange)", fontSize: 12, marginBottom: 8 }}>{t("gpuNoKeyDesc")}</div>
          )}

          <Row
            setting="powerManagementMode"
            label={t("nvcpPowerMode")}
            value={data.powerManagementMode}
            options={[
              ["adaptive", t("nvcpPowerModeAdaptive")],
              ["preferMaxPerformance", t("nvcpPowerModeMaxPerf")],
            ]}
          />
          <Row
            setting="maxPreRenderedFrames"
            label={t("nvcpPrerender")}
            value={data.maxPreRenderedFrames}
            options={[
              ["1", "1"],
              ["2", "2"],
              ["3", `3 (${t("nvcpDefault")})`],
              ["4", "4"],
            ]}
          />
          <Row
            setting="lowLatencyMode"
            label={t("nvcpLowLatency")}
            value={data.lowLatencyMode}
            options={[
              ["off", `${t("nvcpOff")} (${t("nvcpDefault")})`],
              ["on", t("nvcpOn")],
              ["ultra", t("nvcpUltra")],
            ]}
          />
          <Row
            setting="threadedOptimization"
            label={t("nvcpThreadedOpt")}
            value={data.threadedOptimization}
            options={[
              ["off", `${t("nvcpOff")} (${t("nvcpDefault")})`],
              ["on", t("nvcpOn")],
            ]}
          />

          {log && <div className="mono muted" style={{ fontSize: 12, marginTop: 12 }}>{log}</div>}
        </Card>
      )}

      <Card title={t("nvcpMoreSettings")} style={{ marginTop: 14 }}>
        <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>{t("nvcpMoreSettingsDesc")}</div>
        <button className="btn small" disabled={opening} onClick={openPanel}>
          {opening ? <Spinner /> : `↗ ${t("nvcpOpenPanel")}`}
        </button>
      </Card>
    </>
  );
}
