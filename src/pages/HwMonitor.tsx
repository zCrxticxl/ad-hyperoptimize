import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";

// ─── helpers ─────────────────────────────────────────────────────────────────

function TempGauge({ tempC, label }: { tempC: number | null; label: string }) {
  if (tempC == null) return <span className="muted">—</span>;
  const color =
    tempC < 50 ? "var(--green)" :
    tempC < 70 ? "var(--accent)" :
    tempC < 85 ? "var(--orange)" : "var(--red)";
  const pct = Math.min(Math.round((tempC / 100) * 100), 100);
  return (
    <div style={{ marginBottom: 10 }}>
      <div className="row" style={{ justifyContent: "space-between", marginBottom: 3 }}>
        <span style={{ fontSize: 13 }}>{label}</span>
        <span style={{ color, fontWeight: 700 }}>{tempC}°C</span>
      </div>
      <div style={{ background: "var(--bg2)", borderRadius: 4, height: 6 }}>
        <div style={{ width: `${pct}%`, height: "100%", background: color, borderRadius: 4, transition: "width 0.4s" }} />
      </div>
    </div>
  );
}

function WearBar({ pct }: { pct: number | null }) {
  if (pct == null) return <span className="muted">—</span>;
  const color = pct < 50 ? "var(--green)" : pct < 80 ? "var(--orange)" : "var(--red)";
  return (
    <div>
      <div style={{ background: "var(--bg2)", borderRadius: 4, height: 5, marginTop: 3 }}>
        <div style={{ width: `${pct}%`, height: "100%", background: color, borderRadius: 4 }} />
      </div>
      <span style={{ fontSize: 11, color }}>{pct}% worn</span>
    </div>
  );
}

// ─── CPU Temps ────────────────────────────────────────────────────────────────

function CpuTempsCard({ data }: { data: any }) {
  const zones: any[] = data?.cpuZones ?? [];

  return (
    <Card title="CPU Thermal Zones">
      {zones.length === 0 ? (
        <div>
          <div className="muted" style={{ marginBottom: 8 }}>
            {data?.cpuError
              ? `No ACPI thermal data: ${data.cpuError}`
              : "No ACPI thermal zones reported."}
          </div>
          <div className="muted" style={{ fontSize: 12 }}>
            For accurate CPU temps install{" "}
            <span style={{ color: "var(--accent)" }}>HWiNFO64</span> or{" "}
            <span style={{ color: "var(--accent)" }}>LibreHardwareMonitor</span> (free).
            Windows WMI thermal zones are often not exposed on modern motherboards.
          </div>
        </div>
      ) : (
        zones.map((z: any, i: number) => (
          <TempGauge key={i} tempC={z.tempC} label={z.name || `Zone ${i}`} />
        ))
      )}
    </Card>
  );
}

// ─── GPU ──────────────────────────────────────────────────────────────────────

function GpuCard({ data }: { data: any }) {
  const gpus: any[] = data?.gpus ?? [];

  if (gpus.length === 0) {
    return (
      <Card title="GPU">
        <div className="muted" style={{ marginBottom: 8 }}>
          No GPU data — nvidia-smi not found or AMD/Intel GPU.
        </div>
        <div className="muted" style={{ fontSize: 12 }}>
          NVIDIA: install latest Game Ready or Studio drivers (includes nvidia-smi).<br />
          AMD: GPU temps visible via AMD Adrenalin or HWiNFO64.
        </div>
      </Card>
    );
  }

  return (
    <>
      {gpus.map((g: any, i: number) => (
        <Card key={i} title={`GPU · ${g.name ?? "Unknown"}`}>
          <TempGauge tempC={g.tempC} label="Temperature" />
          {g.utilPct != null && (
            <div style={{ marginBottom: 10 }}>
              <div className="row" style={{ justifyContent: "space-between", marginBottom: 3 }}>
                <span style={{ fontSize: 13 }}>GPU utilisation</span>
                <span style={{ fontWeight: 700 }}>{g.utilPct}%</span>
              </div>
              <div style={{ background: "var(--bg2)", borderRadius: 4, height: 6 }}>
                <div style={{ width: `${g.utilPct}%`, height: "100%", background: "var(--accent)", borderRadius: 4 }} />
              </div>
            </div>
          )}
          <table className="tbl"><tbody>
            {g.memUsedMb != null && (
              <tr>
                <td className="muted">VRAM</td>
                <td>{g.memUsedMb} / {g.memTotalMb} MB ({Math.round(g.memUsedMb / g.memTotalMb * 100)}%)</td>
              </tr>
            )}
            {g.powerW != null && <tr><td className="muted">Power draw</td><td>{g.powerW} W</td></tr>}
            {g.clockMhz != null && <tr><td className="muted">Core clock</td><td>{g.clockMhz} MHz</td></tr>}
            <tr><td className="muted">Vendor</td><td>{g.vendor}</td></tr>
          </tbody></table>
        </Card>
      ))}
    </>
  );
}

// ─── SSD S.M.A.R.T. ──────────────────────────────────────────────────────────

const HEALTH_COLOR: Record<string, string> = {
  good:    "var(--green)",
  Healthy: "var(--green)",
  warning: "var(--orange)",
  Warning: "var(--orange)",
  bad:     "var(--red)",
  Unhealthy: "var(--red)",
};

function SmartCard({ data }: { data: any }) {
  const disks: any[] = data?.disks ?? [];

  return (
    <Card title={`Disk Health / S.M.A.R.T. (${disks.length} drives)`}>
      {disks.length === 0 && <div className="muted">No disk data.</div>}
      {disks.map((d: any, i: number) => (
        <div key={i} style={{ marginBottom: i < disks.length - 1 ? 16 : 0, paddingBottom: i < disks.length - 1 ? 16 : 0, borderBottom: i < disks.length - 1 ? "1px solid var(--border)" : "none" }}>
          {d.error ? (
            <div className="muted">{d.error}</div>
          ) : (
            <>
              <div className="row" style={{ marginBottom: 8, alignItems: "center", gap: 10 }}>
                <b>{d.name}</b>
                <span className="muted" style={{ fontSize: 11 }}>{d.type} · {d.bus} · {d.sizeGb} GB</span>
                <span style={{ marginLeft: "auto", color: HEALTH_COLOR[d.health] ?? "var(--fg)", fontWeight: 700 }}>
                  ● {d.health}
                </span>
              </div>
              <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "4px 20px" }}>
                <div>
                  <div className="muted" style={{ fontSize: 11 }}>Temperature</div>
                  {d.tempC != null
                    ? <TempGauge tempC={d.tempC} label="" />
                    : <span className="muted">—</span>}
                </div>
                <div>
                  <div className="muted" style={{ fontSize: 11 }}>Wear level</div>
                  <WearBar pct={d.wearPct} />
                </div>
                <div>
                  <div className="muted" style={{ fontSize: 11 }}>Power-on hours</div>
                  <span>{d.powerOnHours != null ? `${d.powerOnHours} h` : "—"}</span>
                </div>
                <div>
                  <div className="muted" style={{ fontSize: 11 }}>Read errors</div>
                  <span style={{ color: d.uncorrected > 0 ? "var(--red)" : "var(--fg)" }}>
                    {d.readErrors != null ? d.readErrors : "—"}
                    {d.uncorrected > 0 && <span style={{ color: "var(--red)" }}> ({d.uncorrected} uncorrected!)</span>}
                  </span>
                </div>
              </div>
            </>
          )}
        </div>
      ))}
    </Card>
  );
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export default function HwMonitor() {
  const [data, setData] = useState<any>(null);
  const [busy, setBusy] = useState(false);
  const [ts, setTs] = useState<string>("");

  const load = async () => {
    setBusy(true);
    try {
      const d = await api.hwFull();
      setData(d);
      setTs(new Date().toLocaleTimeString());
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    load();
    const id = setInterval(load, 10_000);
    return () => clearInterval(id);
  }, []);

  return (
    <>
      <div className="page-title">🌡 Hardware Monitor</div>
      <div className="page-sub">
        CPU thermal zones · GPU temperatures &amp; utilisation (requires nvidia-smi) · SSD S.M.A.R.T.
        {ts && <> · Updated {ts}</>}
        {" · "}
        <a style={{ color: "var(--accent)", cursor: "pointer" }} onClick={load}>
          {busy ? "refreshing…" : "Refresh now"}
        </a>
        {" (auto-refresh every 10 s)"}
      </div>

      {!data ? (
        <div style={{ display: "flex", alignItems: "center", gap: 12, marginTop: 40 }}>
          <Spinner /> <span className="muted">Loading hardware data…</span>
        </div>
      ) : (
        <>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 14, marginBottom: 14 }}>
            <CpuTempsCard data={data.temps} />
            <GpuCard data={data.temps} />
          </div>
          <SmartCard data={data.smart} />

          {(data.fans?.fans ?? []).length > 0 && (
            <Card title="Fan Speeds" style={{ marginTop: 14 }}>
              {data.fans.fans.map((f: any, i: number) => (
                <div key={i} className="row" style={{ padding: "4px 0", borderBottom: "1px solid var(--border)" }}>
                  <span style={{ flex: 1 }}>{f.name}</span>
                  <span>{f.rpm > 0 ? `${f.rpm} RPM` : "—"}</span>
                </div>
              ))}
            </Card>
          )}

          <div className="muted" style={{ fontSize: 11, marginTop: 14 }}>
            CPU temps via Windows ACPI WMI (may show 0 on some motherboards — install HWiNFO64 for full sensor access).
            GPU data via nvidia-smi (NVIDIA only). S.M.A.R.T. via Windows Storage API.
          </div>
        </>
      )}
    </>
  );
}
