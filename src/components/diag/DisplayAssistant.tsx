import React, { useState } from "react";
import { TKey, useLang } from "../../i18n";
import { displayAdvice, DisplayInputs, positiveNumber } from "../../lib/displayAdvice";

export type AssistantMonitor = { id: string; name: string; hz: number; maxHz: number };
export function DisplayAssistant({ monitors }: { monitors: AssistantMonitor[] }) {
  const { t } = useLang();
  const [monitorId, setMonitorId] = useState("");
  const [gameHz, setGameHz] = useState("");
  const [cap, setCap] = useState("");
  const [baseFps, setBaseFps] = useState("");
  const [inputs, setInputs] = useState<Omit<DisplayInputs, "cap" | "hz" | "belowMax">>({ vrr: "unknown", gameVsync: "unknown", driverVsync: "unknown", driverQueue: "unknown", frameGen: "unknown", limiter: "unknown", latency: "unknown", mode: "unknown", api: "unknown", goal: "smooth" });
  const monitor = monitors.find(m => m.id === monitorId) ?? monitors[0];
  const hz = positiveNumber(gameHz) ?? (monitor?.hz > 0 ? monitor.hz : null);
  const options: Array<[string, TKey]> = [["unknown", "diagUnknown"], ["on", "diagOn"], ["off", "diagOff"]];
  const select = (key: keyof typeof inputs, label: TKey, values: Array<[string, TKey | string]>) => <label className="diag-field" key={key}>
    <span>{t(label)}</span><select value={inputs[key]} onChange={e => setInputs(old => ({ ...old, [key]: e.target.value }))}>
      {values.map(([value, text]) => <option key={value} value={value}>{text.startsWith("display") || text.startsWith("diag") ? t(text as TKey) : text}</option>)}
    </select>
  </label>;
  const unknown: [string, TKey] = ["unknown", "diagUnknown"];
  const advice = displayAdvice({ ...inputs, cap: positiveNumber(cap), hz, belowMax: !!monitor && monitor.maxHz > Math.round(monitor.hz) + 1 });
  return <section className="diag-assistant" aria-label={t("displayAssistant")}>
    <h3>{t("displayAssistant")}</h3><p className="muted">{t("displayAssistantIntro")}</p>
    <div className="diag-fields">
      <label className="diag-field"><span>{t("displayMonitor")}</span><select value={monitor?.id ?? ""} onChange={e => { setMonitorId(e.target.value); setGameHz(""); }}>
        {!monitors.length && <option value="">{t("diagUnknown")}</option>}
        {monitors.map(m => <option key={m.id} value={m.id}>{m.name} ({Math.round(m.hz)} Hz)</option>)}
      </select></label>
      <label className="diag-field"><span>{t("displayGameHz")}</span><input type="number" min="1" max="10000" value={gameHz} placeholder={monitor?.hz ? String(Math.round(monitor.hz)) : ""} onChange={e => setGameHz(e.target.value)} /></label>
      {select("goal", "displayGoal", [["smooth", "displaySmooth"], ["fast", "displayFast"]])}
      {select("vrr", "displayVrr", options)}
      {select("gameVsync", "displayGameVsync", options)}
      {select("driverVsync", "displayDriverVsync", options)}
      {select("limiter", "displayLimiter", [unknown, ["none", "displayNone"], ["game", "displayGame"], ["driver", "displayDriver"], ["external", "displayExternal"], ["automatic", "displayAutomatic"], ["multiple", "displayMultiple"]])}
      <label className="diag-field"><span>{t("displayLimit")}</span><input type="number" min="1" max="10000" value={cap} onChange={e => setCap(e.target.value)} /></label>
      {select("latency", "displayLatency", [unknown, ["none", "displayNone"], ["reflex", "NVIDIA Reflex"], ["antilag", "AMD Anti-Lag / Anti-Lag 2"]])}
      {select("driverQueue", "displayDriverQueue", options)}
      {select("mode", "displayMode", [unknown, ["exclusive", "displayExclusive"], ["borderless", "displayBorderless"], ["windowed", "displayWindowed"]])}
      {select("api", "displayApi", [unknown, ["dx11", "DirectX 11"], ["dx12", "DirectX 12"], ["vulkan", "Vulkan"]])}
      {select("frameGen", "displayFrameGen", options)}
      <label className="diag-field"><span>{t("displayBaseFps")}</span><input type="number" min="1" max="10000" value={baseFps} onChange={e => setBaseFps(e.target.value)} /></label>
    </div>
    {hz && <p className="muted">{t("displayInterval").replace("{ms}", (1000 / hz).toFixed(2))}</p>}
    {positiveNumber(baseFps) && <p className="muted">{t("displayBaseInterval").replace("{ms}", (1000 / positiveNumber(baseFps)!).toFixed(2))}</p>}
    <ul className="diag-advice" aria-live="polite">{advice.map(key => <li key={key}>{t(key)}</li>)}</ul>
  </section>;
}
