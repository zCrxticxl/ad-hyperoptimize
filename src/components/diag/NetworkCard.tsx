import React, { useEffect, useRef, useState } from "react";
import { api } from "../../api";
import { useLang } from "../../i18n";
import { Card } from "../ui";
import type { NetworkPhase, NetworkResult } from "../../lib/diagnosticTypes";

export function NetworkCard() {
  const { t } = useLang();
  const [busy, setBusy] = useState(false);
  const [stopping, setStopping] = useState(false);
  const [runId, setRunId] = useState<string | null>(null);
  const [error, setError] = useState("");
  const [result, setResult] = useState<NetworkResult | null>(null);
  const alive = useRef(true);
  const starting = useRef(false);
  const run = useRef<string | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
      if (timer.current) clearTimeout(timer.current);
      if (run.current) void api.networkCancel(run.current).catch(e => console.error("Network cancellation:", e));
    };
  }, []);
  const start = async (load: boolean) => {
    if (starting.current || (load && !window.confirm(t("networkConsent")))) return;
    starting.current = true; setBusy(true); setStopping(false); setError(""); setResult(null);
    try {
      const id = await api.networkStart(load);
      if (!alive.current) { await api.networkCancel(id); return; }
      run.current = id;
      setRunId(id);
      const poll = async () => {
        try {
          const status = await api.networkStatus(id);
          if (!alive.current || run.current !== id) return;
          if (status.done) {
            setResult(status.result); setError(status.error ?? "");
            setBusy(false); setStopping(false); setRunId(null); starting.current = false; run.current = null;
          } else { timer.current = setTimeout(() => void poll(), 400); }
        } catch (e) {
          if (alive.current) setError(String(e));
          await api.networkCancel(id).catch(cancelError => { if (alive.current) setError(`${String(e)}; ${String(cancelError)}`); });
          if (run.current === id) { run.current = null; starting.current = false; if (alive.current) { setRunId(null); setBusy(false); } }
        }
      };
      void poll();
    } catch (e) { if (alive.current) { setError(String(e)); setBusy(false); } starting.current = false; }
  };
  const cancel = async () => {
    if (!run.current) return;
    setStopping(true);
    try { await api.networkCancel(run.current); }
    catch (e) { setError(String(e)); setStopping(false); }
  };
  const metric = (n: number | null | undefined, unit = "") => n == null || !Number.isFinite(n) ? "?" : `${n.toFixed(1)}${unit}`;
  const phase = (p: NetworkPhase, loaded: boolean) => <section>
    <h3>{t(loaded ? "networkPhaseLoad" : "networkPhaseIdle")}</h3>
    <div className="diag-table"><table className="tbl"><thead><tr>{["networkTarget", "networkReplies", "networkLoss", "networkAverage", "networkJitter", "networkMaximum", "networkErrors"].map(key => <th key={key} scope="col">{t(key as Parameters<typeof t>[0])}</th>)}</tr></thead>
      <tbody>{p.stats.map(s => <tr key={s.target}><td>{s.target === result?.gateway ? `${t("networkGateway")} (${s.target})` : s.target}</td><td>{s.received}/{s.sent}</td><td>{metric(s.lossPct, "%")}</td><td>{metric(s.avgMs, " ms")}</td><td>{metric(s.jitterMs, " ms")}</td><td>{metric(s.maxMs, " ms")}</td><td>{s.errors}</td></tr>)}</tbody></table></div>
    <p className="muted">{t("networkTraffic")}: {metric(p.rxMbps)} / {metric(p.txMbps)} Mbit/s ({metric(p.seconds)} s)</p>
    {loaded && p.stats.map(s => {
      const idle = result?.measurements.idle.stats.find(i => i.target === s.target);
      const enough = s.received >= 5 && (idle?.received ?? 0) >= 5 && s.avgMs != null && idle?.avgMs != null;
      return <p key={s.target} className="muted">{s.target}: {enough ? `${t("networkDelta")}: ${metric(s.avgMs! - idle!.avgMs!, " ms")}` : t("networkInsufficient")}</p>;
    })}
    {loaded && !p.stats.length && <p className="diag-warning">{t("networkInsufficient")}</p>}
  </section>;
  return <Card title={t("networkTitle")}>
    <p className="muted">{t("networkIntro")}</p>
    <div className="row" style={{ flexWrap: "wrap" }}><button className="btn" disabled={busy} onClick={() => void start(false)}>{t("networkIdle")}</button><button className="btn ghost" disabled={busy} onClick={() => void start(true)}>{t("networkLoad")}</button>{busy && <button className="btn ghost" disabled={stopping || !runId} onClick={() => void cancel()}>{stopping ? "…" : t("networkCancel")}</button>}</div>
    {busy && <p role="status">{t("networkRunning")}</p>}
    {error && <p role="alert" className={error === "cancelled" ? "muted" : "diag-error"}>{error === "cancelled" ? t("networkCancelled") : error}</p>}
    {result && <>
      <p><b>{t("networkAdapter")}:</b> {result.adapter} ({result.description})<br /><b>{t("networkGateway")}:</b> {result.gateway}<br /><b>{t("networkLink")}:</b> {result.linkSpeed}</p>
      {phase(result.measurements.idle, false)}
      {result.measurements.loaded && <>{phase(result.measurements.loaded, true)}<p>{t("networkDownloaded")}: {(result.measurements.downloadBytes / 1048576).toFixed(2)} MiB / {metric(result.measurements.downloadSeconds)} s {result.measurements.byteLimitReached && `(${t("networkLimitReached")})`}</p><p className="muted">{t("networkLoadNote")}</p></>}
      {result.measurements.loadError && <p className="diag-error" role="alert">{result.measurements.loadError}</p>}
      {(result.wifiDetails || result.wifiError) && <details><summary>{t("networkWifi")}</summary><p className="muted">{t("networkWifiNote")}</p><pre className="diag-raw">{result.wifiDetails ?? result.wifiError}</pre></details>}
      <p className="muted">{t("networkTrafficNote")}</p><p className="muted">{t("networkDefinition")}</p><p className="muted">{t("networkInterpret")}</p>
    </>}
  </Card>;
}
