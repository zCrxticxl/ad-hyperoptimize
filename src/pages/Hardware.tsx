import React, { useEffect, useState } from "react";
import { api, fmtBytes, fmtAge } from "../api";
import { Card, Spinner, RawJson } from "../components/ui";
import { useLang } from "../i18n";
import type { Mode } from "../App";

const asArr = (v: any) => (Array.isArray(v) ? v : v && !v.error ? [v] : []);

export default function Hardware({ mode }: { mode: Mode }) {
  const { t } = useLang();
  const [scan, setScan] = useState<any | null>(null);
  const [meta, setMeta] = useState<{ time: string; fromCache: boolean } | null>(null);
  const [busy, setBusy] = useState(false);
  const [boot, setBoot] = useState<any | null>(null);
  const [net, setNet] = useState<any | null>(null);
  const [dns, setDns] = useState<any | null>(null);

  const load = (force: boolean) => {
    setBusy(true);
    api
      .fullScan(force)
      .then((env) => {
        setScan(env.data);
        setMeta({ time: env.time, fromCache: env.fromCache });
      })
      .catch((e) => setScan({ error: String(e) }))
      .finally(() => setBusy(false));
  };

  useEffect(() => {
    load(false); // cached → instant after first run
  }, []);

  if (!scan)
    return (
      <>
        <div className="page-title">{t("hwTitle")}</div>
        <div className="page-sub">{t("hwSub")}</div>
        <Spinner /> <span className="muted">{t("hwScanningFirstRun")}</span>
      </>
    );

  const cpu = asArr(scan.cpu)[0] ?? {};
  const os = scan.os ?? {};

  return (
    <>
      <div className="page-title">{t("hwTitle")}</div>
      <div className="page-sub">
        {meta && <>{t("hwScanned")} {fmtAge(meta.time)}{meta.fromCache ? ` (${t("hwCached")})` : ""} · </>}
        <a style={{ color: "var(--accent)", cursor: "pointer" }} onClick={() => load(true)}>
          {busy ? t("hwRescanning") : t("hwRescanNow")}
        </a>
      </div>

      <div className="grid grid-2">
        <Card title={t("hwOperatingSystem")}>
          <table className="tbl">
            <tbody>
              <tr><td className="muted">{t("hwEdition")}</td><td>{os.Caption}</td></tr>
              <tr><td className="muted">{t("hwBuild")}</td><td>{os.Version} ({os.BuildNumber})</td></tr>
              <tr><td className="muted">{t("hwArchitecture")}</td><td>{os.OSArchitecture}</td></tr>
              <tr><td className="muted">{t("hwRam")}</td><td>{os.TotalVisibleMemorySize ? (os.TotalVisibleMemorySize / 1048576).toFixed(1) + ` GB ${t("hwTotal")}, ` + (os.FreePhysicalMemory / 1048576).toFixed(1) + ` GB ${t("hwFree")}` : "—"}</td></tr>
            </tbody>
          </table>
        </Card>
        <Card title={t("hwCpu")}>
          <table className="tbl">
            <tbody>
              <tr><td className="muted">{t("hwModel")}</td><td>{cpu.Name}</td></tr>
              <tr><td className="muted">{t("hwCoresThreads")}</td><td>{cpu.NumberOfCores} / {cpu.NumberOfLogicalProcessors}</td></tr>
              <tr><td className="muted">{t("hwClock")}</td><td>{cpu.CurrentClockSpeed} MHz ({t("hwMax")} {cpu.MaxClockSpeed})</td></tr>
              <tr><td className="muted">{t("hwL3Cache")}</td><td>{cpu.L3CacheSize ? (cpu.L3CacheSize / 1024).toFixed(0) + " MB" : "—"}</td></tr>
              <tr><td className="muted">{t("hwVirtualization")}</td><td>{String(cpu.VirtualizationFirmwareEnabled ?? "—")}</td></tr>
            </tbody>
          </table>
        </Card>
      </div>

      <div className="grid grid-2 mt">
        <Card title={t("hwGpu")}>
          {asArr(scan.gpu).map((g: any, i: number) => (
            <table className="tbl" key={i}>
              <tbody>
                <tr><td className="muted">{t("hwAdapter")}</td><td>{g.Name}</td></tr>
                <tr><td className="muted">{t("hwDriver")}</td><td>{g.DriverVersion}</td></tr>
                <tr><td className="muted">{t("hwMode")}</td><td>{g.VideoModeDescription} @ {g.CurrentRefreshRate} Hz</td></tr>
              </tbody>
            </table>
          ))}
        </Card>
        <Card title={t("hwMotherboardBios")}>
          <table className="tbl">
            <tbody>
              <tr><td className="muted">{t("hwBoard")}</td><td>{scan.board?.Manufacturer} {scan.board?.Product}</td></tr>
              <tr><td className="muted">{t("hwBios")}</td><td>{scan.bios?.Manufacturer} {scan.bios?.SMBIOSBIOSVersion}</td></tr>
            </tbody>
          </table>
        </Card>
      </div>

      <Card title={t("hwStorageSmart")} style={{ marginTop: 14 }}>
        <table className="tbl">
          <thead><tr><th>{t("hwDisk")}</th><th>{t("hwType")}</th><th>{t("hwBus")}</th><th>{t("hwSize")}</th><th>{t("hwHealth")}</th></tr></thead>
          <tbody>
            {asArr(scan.disks).map((d: any, i: number) => (
              <tr key={i}>
                <td>{d.FriendlyName}</td><td>{d.MediaType}</td><td>{d.BusType}</td>
                <td>{d.Size ? fmtBytes(d.Size) : "—"}</td>
                <td style={{ color: d.HealthStatus === "Healthy" ? "var(--green)" : "var(--red)" }}>{d.HealthStatus}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {asArr(scan.smart).length > 0 && (
          <table className="tbl mt">
            <thead><tr><th>{t("hwSmartDev")}</th><th>{t("hwTempC")}</th><th>{t("hwWearPct")}</th><th>{t("hwReadErrs")}</th><th>{t("hwPowerOnHrs")}</th></tr></thead>
            <tbody>
              {asArr(scan.smart).map((s: any, i: number) => (
                <tr key={i}>
                  <td>{s.DeviceId}</td><td>{s.Temperature ?? "n/a"}</td><td>{s.Wear ?? "n/a"}</td>
                  <td>{s.ReadErrorsTotal ?? "n/a"}</td><td>{s.PowerOnHours ?? "n/a"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </Card>

      <div className="grid grid-2 mt">
        <Card title={t("hwBootTimeAnalysis")}>
          {!boot ? (
            <button className="btn ghost small" onClick={() => api.bootAnalysis().then(setBoot)}>{t("hwAnalyzeBootHistory")}</button>
          ) : (
            <table className="tbl">
              <thead><tr><th>{t("hwBoot")}</th><th>{t("hwDuration")}</th></tr></thead>
              <tbody>
                {asArr(boot).map((b: any, i: number) => (
                  <tr key={i}><td>{b.time}</td><td>{(b.bootMs / 1000).toFixed(1)} s</td></tr>
                ))}
                {asArr(boot).length === 0 && <tr><td colSpan={2} className="muted">{t("hwNoBootEvents")}</td></tr>}
              </tbody>
            </table>
          )}
        </Card>
        <Card title={t("hwNetworkDiagnostics")}>
          <div className="row">
            <button className="btn ghost small" onClick={() => api.networkDiag().then(setNet)}>{t("hwLatencyPacketLoss")}</button>
            <button className="btn ghost small" onClick={() => api.dnsBenchmark().then(setDns)}>{t("hwDnsBenchmark")}</button>
          </div>
          {net && !net.error && (
            <div className="mt">{t("hwAvg")} <b>{net.avgMs} ms</b> · {t("hwMin")} {net.minMs} · {t("hwMax")} {net.maxMs} · {t("hwLoss")} <b>{net.lossPct}%</b></div>
          )}
          {net?.error && <div className="mt" style={{ color: "var(--yellow)" }}>{net.error}</div>}
          {dns && (
            <table className="tbl mt">
              <tbody>
                {asArr(dns).map((d: any, i: number) => (
                  <tr key={i}><td>{d.server}</td><td>{d.ms} ms</td></tr>
                ))}
              </tbody>
            </table>
          )}
        </Card>
      </div>

      {mode === "expert" && (
        <Card title={t("hwFullScanDataExpert")} style={{ marginTop: 14 }}>
          <RawJson label={t("hwCompleteScanJson")} data={scan} />
        </Card>
      )}
    </>
  );
}
