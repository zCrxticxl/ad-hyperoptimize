import React, { useEffect, useState } from "react";
import { api, fmtBytes, fmtAge } from "../api";
import { Card, Spinner, RawJson } from "../components/ui";
import type { Mode } from "../App";

const asArr = (v: any) => (Array.isArray(v) ? v : v && !v.error ? [v] : []);

export default function Hardware({ mode }: { mode: Mode }) {
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
        <div className="page-title">Hardware & System</div>
        <div className="page-sub">Deep inventory via WMI, SMART, PowerShell and the event log.</div>
        <Spinner /> <span className="muted">Scanning (10–30s on first run)…</span>
      </>
    );

  const cpu = asArr(scan.cpu)[0] ?? {};
  const os = scan.os ?? {};

  return (
    <>
      <div className="page-title">Hardware & System</div>
      <div className="page-sub">
        {meta && <>Scanned {fmtAge(meta.time)}{meta.fromCache ? " (cached)" : ""} · </>}
        <a style={{ color: "var(--accent)", cursor: "pointer" }} onClick={() => load(true)}>
          {busy ? "rescanning…" : "Re-scan now"}
        </a>
      </div>

      <div className="grid grid-2">
        <Card title="Operating system">
          <table className="tbl">
            <tbody>
              <tr><td className="muted">Edition</td><td>{os.Caption}</td></tr>
              <tr><td className="muted">Build</td><td>{os.Version} ({os.BuildNumber})</td></tr>
              <tr><td className="muted">Architecture</td><td>{os.OSArchitecture}</td></tr>
              <tr><td className="muted">RAM</td><td>{os.TotalVisibleMemorySize ? (os.TotalVisibleMemorySize / 1048576).toFixed(1) + " GB total, " + (os.FreePhysicalMemory / 1048576).toFixed(1) + " GB free" : "—"}</td></tr>
            </tbody>
          </table>
        </Card>
        <Card title="CPU">
          <table className="tbl">
            <tbody>
              <tr><td className="muted">Model</td><td>{cpu.Name}</td></tr>
              <tr><td className="muted">Cores / Threads</td><td>{cpu.NumberOfCores} / {cpu.NumberOfLogicalProcessors}</td></tr>
              <tr><td className="muted">Clock</td><td>{cpu.CurrentClockSpeed} MHz (max {cpu.MaxClockSpeed})</td></tr>
              <tr><td className="muted">L3 cache</td><td>{cpu.L3CacheSize ? (cpu.L3CacheSize / 1024).toFixed(0) + " MB" : "—"}</td></tr>
              <tr><td className="muted">Virtualization</td><td>{String(cpu.VirtualizationFirmwareEnabled ?? "—")}</td></tr>
            </tbody>
          </table>
        </Card>
      </div>

      <div className="grid grid-2 mt">
        <Card title="GPU">
          {asArr(scan.gpu).map((g: any, i: number) => (
            <table className="tbl" key={i}>
              <tbody>
                <tr><td className="muted">Adapter</td><td>{g.Name}</td></tr>
                <tr><td className="muted">Driver</td><td>{g.DriverVersion}</td></tr>
                <tr><td className="muted">Mode</td><td>{g.VideoModeDescription} @ {g.CurrentRefreshRate} Hz</td></tr>
              </tbody>
            </table>
          ))}
        </Card>
        <Card title="Motherboard & BIOS">
          <table className="tbl">
            <tbody>
              <tr><td className="muted">Board</td><td>{scan.board?.Manufacturer} {scan.board?.Product}</td></tr>
              <tr><td className="muted">BIOS</td><td>{scan.bios?.Manufacturer} {scan.bios?.SMBIOSBIOSVersion}</td></tr>
            </tbody>
          </table>
        </Card>
      </div>

      <Card title="Storage & SMART" style={{ marginTop: 14 }}>
        <table className="tbl">
          <thead><tr><th>Disk</th><th>Type</th><th>Bus</th><th>Size</th><th>Health</th></tr></thead>
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
            <thead><tr><th>SMART dev</th><th>Temp °C</th><th>Wear %</th><th>Read errs</th><th>Power-on hrs</th></tr></thead>
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
        <Card title="Boot time analysis">
          {!boot ? (
            <button className="btn ghost small" onClick={() => api.bootAnalysis().then(setBoot)}>Analyze boot history</button>
          ) : (
            <table className="tbl">
              <thead><tr><th>Boot</th><th>Duration</th></tr></thead>
              <tbody>
                {asArr(boot).map((b: any, i: number) => (
                  <tr key={i}><td>{b.time}</td><td>{(b.bootMs / 1000).toFixed(1)} s</td></tr>
                ))}
                {asArr(boot).length === 0 && <tr><td colSpan={2} className="muted">No boot perf events found.</td></tr>}
              </tbody>
            </table>
          )}
        </Card>
        <Card title="Network diagnostics">
          <div className="row">
            <button className="btn ghost small" onClick={() => api.networkDiag().then(setNet)}>Latency / packet loss</button>
            <button className="btn ghost small" onClick={() => api.dnsBenchmark().then(setDns)}>DNS benchmark</button>
          </div>
          {net && !net.error && (
            <div className="mt">Avg <b>{net.avgMs} ms</b> · min {net.minMs} · max {net.maxMs} · loss <b>{net.lossPct}%</b></div>
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
        <Card title="Full scan data (expert)" style={{ marginTop: 14 }}>
          <RawJson label="Complete scan JSON" data={scan} />
        </Card>
      )}
    </>
  );
}
