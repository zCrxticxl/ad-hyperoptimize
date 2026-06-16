import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { HwWarnings, RiskBadge, RiskNotice } from "../components/HwWarnings";
import { useHwProfile } from "../hooks/useHwProfile";

// ─────────────────────────────────────────────────────────────────────────────

function mb(v: number | null | undefined) {
  if (v == null) return "—";
  if (v >= 1024) return `${(v / 1024).toFixed(1)} GB`;
  return `${v} MB`;
}

function TempBar({ pct, color }: { pct: number; color: string }) {
  return (
    <div style={{ background: "var(--bg2)", borderRadius: 4, height: 6, width: "100%", marginTop: 3 }}>
      <div style={{ background: color, width: `${Math.min(pct, 100)}%`, height: "100%", borderRadius: 4, transition: "width 0.3s" }} />
    </div>
  );
}

// ─── Timer Resolution ─────────────────────────────────────────────────────────

function TimerCard({ admin }: { admin: boolean }) {
  const [data, setData] = useState<any>(null);
  const [busy, setBusy] = useState(false);
  const [log, setLog] = useState("");
  const [riskAck, setRiskAck] = useState(false);
  const profile = useHwProfile();
  const minRisk = profile?.tweakRisks?.["timer_resolution_min"];
  const minNeedsAck = !!minRisk && !riskAck;

  const load = async () => {
    setData(await api.timerGet());
  };

  useEffect(() => { load(); }, []);

  const apply = async (val100ns: number) => {
    setBusy(true);
    try { setLog(await api.timerSet(val100ns)); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  const reset = async () => {
    setBusy(true);
    try { setLog(await api.timerReset()); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  const isMin = data && data.currentMs <= 0.55;
  const is1ms = data && data.currentMs <= 1.05;
  const currentColor = isMin ? "var(--green)" : is1ms ? "var(--accent)" : "var(--red)";

  return (
    <Card title="Timer Resolution">
      <div className="muted" style={{ fontSize: 12, marginBottom: 4 }}>
        Windows default is ~15.6 ms. Lowering to 0.5 ms reduces scheduler jitter — measurably improves input latency and frame pacing.
      </div>
      <div style={{ marginBottom: 10 }}><RiskBadge id="timer_resolution_min" /></div>
      {!data ? <Spinner /> : data.error ? (
        <div className="muted">{data.error}</div>
      ) : (
        <>
          <table className="tbl" style={{ marginBottom: 10 }}><tbody>
            <tr>
              <td className="muted">Current</td>
              <td><span style={{ color: currentColor, fontWeight: 700 }}>
                {data.currentMs != null ? `${data.currentMs} ms` : "—"}
              </span></td>
            </tr>
            <tr><td className="muted">Min possible</td><td>{data.minMs != null ? `${data.minMs} ms` : "—"}</td></tr>
            <tr><td className="muted">Max (default)</td><td>{data.maxMs != null ? `${data.maxMs} ms` : "—"}</td></tr>
            <tr>
              <td className="muted">Persistent</td>
              <td>
                <span style={{ color: data.persistent ? "var(--green)" : data.globalReqEnabled ? "var(--accent)" : "var(--red)" }}>
                  {data.persistent
                    ? "✓ active (background process running)"
                    : data.globalReqEnabled
                      ? "~ configured — apply again or reboot"
                      : "✗ not set"}
                </span>
              </td>
            </tr>
          </tbody></table>
          {!admin && <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>⚠ Requires admin rights to apply.</div>}
          <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
            {minNeedsAck ? (
              <button className="btn small" style={{ background: "var(--red)", borderColor: "var(--red)" }} disabled={busy || !admin} onClick={() => setRiskAck(true)}>
                ⚠ Review risk before setting 0.5 ms
              </button>
            ) : (
              <button className="btn small" onClick={() => apply(5000)} disabled={busy || !admin}>
                ⚡ Set 0.5 ms (gaming)
              </button>
            )}
            <button className="btn small ghost" onClick={() => apply(10000)} disabled={busy || !admin}>
              Set 1.0 ms
            </button>
            <button className="btn small ghost" onClick={reset} disabled={busy || !admin}>
              ↩ Reset default
            </button>
            <button className="btn small ghost" onClick={load} disabled={busy}>↺</button>
          </div>
          {minRisk && <RiskNotice id="timer_resolution_min" />}
          {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 8 }}>{log}</div>}
        </>
      )}
    </Card>
  );
}

// ─── MSI Mode ─────────────────────────────────────────────────────────────────

function MsiCard({ admin }: { admin: boolean }) {
  const [data, setData] = useState<any>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [log, setLog] = useState("");

  const load = async () => setData(await api.msiList());
  useEffect(() => { load(); }, []);

  const toggle = async (dev: any) => {
    setBusy(dev.regPath);
    try { setLog(await api.msiSet(dev.regPath, !dev.msiEnabled)); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(null); }
  };

  const devices: any[] = data?.devices ?? [];

  return (
    <Card title="MSI Mode (Message Signaled Interrupts)">
      <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>
        MSI replaces legacy line-based interrupts with in-band PCIe messages — eliminates shared interrupt contention,
        reduces DPC latency on GPU and NIC. Requires reboot after change.
      </div>
      {!data ? <Spinner /> : (
        <>
          {devices.length === 0 && <div className="muted">No GPU/NIC devices found.</div>}
          {devices.map((d: any, i: number) => (
            <div key={i} className="row" style={{ padding: "6px 0", borderBottom: "1px solid var(--border)", alignItems: "center", gap: 10 }}>
              <span style={{ color: "var(--muted)", fontSize: 11, width: 36, flexShrink: 0 }}>[{d.type}]</span>
              <span style={{ flex: 1 }}>{d.name}</span>
              <span style={{ color: d.msiEnabled ? "var(--green)" : "var(--red)", fontWeight: 700, fontSize: 12 }}>
                {d.msiEnabled ? "● MSI ON" : "● LINE"}
              </span>
              <RiskBadge id="msi_mode_toggle" />
              <button
                className={`btn small ${d.msiEnabled ? "ghost" : ""}`}
                onClick={() => toggle(d)}
                disabled={busy === d.regPath || !admin}
              >
                {busy === d.regPath ? "…" : d.msiEnabled ? "Disable" : "Enable MSI"}
              </button>
            </div>
          ))}
          {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 8 }}>⚠ {log}</div>}
          <div className="muted" style={{ fontSize: 11, marginTop: 8 }}>
            Reboot required after any change. Verify in Device Manager → Interrupt Mode.
          </div>
        </>
      )}
    </Card>
  );
}

// ─── Network Adapter Tweaks ────────────────────────────────────────────────────

const NET_PROPS: { key: string; label: string; keyword: string; offVal: number; onVal: number }[] = [
  { key: "intMod",     label: "Interrupt Moderation",       keyword: "*InterruptModeration",      offVal: 0, onVal: 1 },
  { key: "rss",        label: "Receive Side Scaling (RSS)", keyword: "*RSS",                       offVal: 0, onVal: 1 },
  { key: "tcpOffload", label: "TCP Checksum Offload",       keyword: "*TCPChecksumOffloadIPv4",   offVal: 0, onVal: 1 },
  { key: "lsoV2",      label: "Large Send Offload v2",      keyword: "*LsoV2IPv4",                offVal: 0, onVal: 1 },
];

function NetCard({ admin }: { admin: boolean }) {
  const [data, setData] = useState<any>(null);
  const [busy, setBusy] = useState(false);
  const [log, setLog] = useState("");

  const load = async () => setData(await api.netAdapters());
  useEffect(() => { load(); }, []);

  const tweak = async (adapter: string, keyword: string, value: number) => {
    setBusy(true);
    try { setLog(await api.netTweak(adapter, keyword, value)); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  const allGaming = async () => {
    setBusy(true);
    try { setLog(await api.netTweakAllGaming()); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  const resetAll = async () => {
    if (!window.confirm("Reset all adapter properties to driver defaults?")) return;
    setBusy(true);
    try { setLog(await api.netResetAll()); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  const adapters: any[] = data?.adapters ?? [];

  const isDisabled = (val: string | null | undefined) =>
    val == null || val === "" || val === "0" || val?.toLowerCase().includes("disab");

  return (
    <Card title="Network Adapter Tweaks">
      <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>
        Disabling interrupt moderation and offloading reduces latency at the cost of slightly more CPU usage — ideal for gaming/low-latency workloads.
      </div>
      {!data ? <Spinner /> : (
        <>
          {adapters.length === 0 && <div className="muted">No active physical adapters found.</div>}
          {adapters.map((a: any, ai: number) => (
            <div key={ai} style={{ marginBottom: 14 }}>
              <div className="row" style={{ marginBottom: 6, alignItems: "center", gap: 8 }}>
                <b>{a.name}</b>
                <span className="muted" style={{ fontSize: 11 }}>{a.description}</span>
                {a.speedMbps > 0 && <span className="muted" style={{ fontSize: 11 }}>{a.speedMbps} Mbps</span>}
                {a.status && a.status !== "Up" && (
                  <span style={{ fontSize: 11, color: "var(--orange)" }}>[{a.status}]</span>
                )}
              </div>
              {NET_PROPS.map((p) => {
                const raw = a[p.key];
                const off = isDisabled(raw);
                return (
                  <div key={p.key} className="row" style={{ padding: "3px 0", borderBottom: "1px solid var(--border)", alignItems: "center", gap: 8 }}>
                    <span style={{ flex: 1, fontSize: 13 }}>{p.label}</span>
                    <span className="muted" style={{ fontSize: 11, minWidth: 70, textAlign: "right" }}>{raw ?? "—"}</span>
                    <span style={{ color: off ? "var(--green)" : "var(--muted)", fontWeight: 700, fontSize: 11, minWidth: 24 }}>
                      {off ? "●" : "○"}
                    </span>
                    <RiskBadge id="net_offload_disable" />
                    <button
                      className="btn small ghost"
                      style={{ minWidth: 70 }}
                      disabled={busy || !admin}
                      onClick={() => tweak(a.name, p.keyword, off ? p.onVal : p.offVal)}
                    >
                      {off ? "Enable" : "Disable"}
                    </button>
                  </div>
                );
              })}
            </div>
          ))}
          {!admin && <div className="muted" style={{ fontSize: 12 }}>⚠ Requires admin rights.</div>}
          <div className="row" style={{ gap: 8, marginTop: 10 }}>
            <button className="btn small" onClick={allGaming} disabled={busy || !admin}>
              ⚡ Disable all (gaming preset)
            </button>
            <button className="btn small ghost" onClick={resetAll} disabled={busy || !admin}>
              ↩ Reset all to defaults
            </button>
            <button className="btn small ghost" onClick={load} disabled={busy}>↺</button>
          </div>
          {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 8 }}>{log}</div>}
        </>
      )}
    </Card>
  );
}

// ─── RAM Standby Cleaner ──────────────────────────────────────────────────────

function RamCard({ admin }: { admin: boolean }) {
  const [data, setData] = useState<any>(null);
  const [busy, setBusy] = useState(false);
  const [log, setLog] = useState("");

  const load = async () => setData(await api.ramInfo());
  useEffect(() => { load(); }, []);

  const flush = async () => {
    setBusy(true);
    try { setLog(await api.ramFlushStandby()); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  const standbyPct = data?.totalMb > 0 ? Math.round((data.standbyMb / data.totalMb) * 100) : 0;
  const usedPct    = data?.totalMb > 0 ? Math.round((data.usedMb    / data.totalMb) * 100) : 0;

  return (
    <Card title="RAM Standby Cleaner">
      <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>
        Standby cache holds recently freed pages — Windows reclaims it automatically, but flushing can immediately free RAM
        for latency-sensitive workloads (e.g. before launching a game).
      </div>
      {!data ? <Spinner /> : data.error ? (
        <div className="muted">{data.error}</div>
      ) : (
        <>
          <table className="tbl" style={{ marginBottom: 10 }}><tbody>
            <tr><td className="muted">Total RAM</td><td>{mb(data.totalMb)}</td></tr>
            <tr>
              <td className="muted">In use</td>
              <td>
                {mb(data.usedMb)} ({usedPct}%)
                <TempBar pct={usedPct} color={usedPct > 85 ? "var(--red)" : "var(--accent)"} />
              </td>
            </tr>
            <tr>
              <td className="muted">Standby cache</td>
              <td>
                {mb(data.standbyMb)} ({standbyPct}%)
                <TempBar pct={standbyPct} color="var(--muted)" />
              </td>
            </tr>
            <tr><td className="muted">Modified list</td><td>{mb(data.modifiedMb)}</td></tr>
            <tr><td className="muted">Free</td><td style={{ color: "var(--green)" }}>{mb(data.freeMb)}</td></tr>
          </tbody></table>
          <div className="row" style={{ gap: 8, alignItems: "center" }}>
            <button className="btn small" onClick={flush} disabled={busy || !admin}>
              {busy ? <><Spinner /> Flushing…</> : "⚡ Flush Standby List"}
            </button>
            <button className="btn small ghost" onClick={load} disabled={busy}>↺ Refresh</button>
            <RiskBadge id="ram_flush_standby" />
          </div>
          {!admin && <div className="muted" style={{ fontSize: 12, marginTop: 6 }}>⚠ Requires admin rights.</div>}
          {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 8 }}>{log}</div>}
        </>
      )}
    </Card>
  );
}

// ─── Pagefile ─────────────────────────────────────────────────────────────────

function PagefileCard({ admin }: { admin: boolean }) {
  const [data, setData] = useState<any>(null);
  const [busy, setBusy] = useState(false);
  const [log, setLog] = useState("");
  const [customPath, setCustomPath] = useState("C:\\pagefile.sys");
  const [customInit, setCustomInit] = useState("4096");
  const [customMax, setCustomMax] = useState("8192");
  const [showCustom, setShowCustom] = useState(false);
  const profile = useHwProfile();
  const pfRisk = profile?.tweakRisks?.["pagefile_disable"];

  const load = async () => {
    const d = await api.pagefileInfo();
    setData(d);
    if (d?.files?.length > 0) setCustomPath(d.files[0].path);
  };
  useEffect(() => { load(); }, []);

  const act = async (fn: () => Promise<string>) => {
    setBusy(true);
    try { setLog(await fn()); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  const files: any[] = data?.files ?? [];

  return (
    <Card title="Pagefile Manager">
      <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>
        Fixed-size pagefile eliminates OS resizing overhead. Disable entirely only if you have 32 GB+ RAM —
        some apps and crash dumps still require a pagefile. All changes need a reboot.
      </div>
      {!data ? <Spinner /> : data.error ? (
        <div className="muted">{data.error}</div>
      ) : (
        <>
          <table className="tbl" style={{ marginBottom: 10 }}><tbody>
            <tr><td className="muted">RAM installed</td><td>{data.ramGb} GB</td></tr>
            <tr>
              <td className="muted">Mode</td>
              <td style={{ color: data.autoManaged ? "var(--accent)" : "var(--fg)" }}>
                {data.autoManaged ? "Auto-managed (Windows default)" : "Custom / Disabled"}
              </td>
            </tr>
            {files.map((f: any, i: number) => (
              <React.Fragment key={i}>
                <tr><td className="muted">Path</td><td className="mono">{f.path}</td></tr>
                <tr><td className="muted">Size</td><td>{f.initialMb}–{f.maxMb} MB (peak: {f.peakMb} MB)</td></tr>
              </React.Fragment>
            ))}
            {files.length === 0 && !data.autoManaged && (
              <tr><td className="muted" colSpan={2}>No pagefile (disabled)</td></tr>
            )}
          </tbody></table>

          {!admin && <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>⚠ Requires admin rights.</div>}

          <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
            <button className="btn small ghost" onClick={() => act(api.pagefileSetAuto)} disabled={busy || !admin}>
              ↩ Auto (Windows default)
            </button>
            <button className="btn small ghost" onClick={() => setShowCustom(s => !s)} disabled={!admin}>
              ✎ Custom size…
            </button>
            <button
              className="btn small danger"
              onClick={() => {
                if (pfRisk && !window.confirm(`${pfRisk.title}\n\n${pfRisk.message}\n\nContinue anyway?`)) return;
                if (!pfRisk && data.ramGb < 28 && !window.confirm(`You only have ${data.ramGb} GB RAM. Disabling the pagefile can cause system instability. Continue?`)) return;
                act(api.pagefileDisable);
              }}
              disabled={busy || !admin}
            >
              ⛔ Disable pagefile
            </button>
          </div>
          <div style={{ marginTop: 6 }}><RiskBadge id="pagefile_disable" /></div>
          {pfRisk && <RiskNotice id="pagefile_disable" />}

          {showCustom && (
            <div style={{ marginTop: 12, padding: 10, background: "var(--bg2)", borderRadius: 6, border: "1px solid var(--border)" }}>
              <div className="row" style={{ gap: 8, flexWrap: "wrap", alignItems: "center" }}>
                <div>
                  <div className="muted" style={{ fontSize: 11 }}>Path</div>
                  <input
                    className="input"
                    value={customPath}
                    onChange={e => setCustomPath(e.target.value)}
                    style={{ width: 160, padding: "3px 6px", fontSize: 12, background: "var(--bg)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}
                  />
                </div>
                <div>
                  <div className="muted" style={{ fontSize: 11 }}>Initial MB</div>
                  <input
                    className="input" type="number"
                    value={customInit}
                    onChange={e => setCustomInit(e.target.value)}
                    style={{ width: 90, padding: "3px 6px", fontSize: 12, background: "var(--bg)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}
                  />
                </div>
                <div>
                  <div className="muted" style={{ fontSize: 11 }}>Max MB</div>
                  <input
                    className="input" type="number"
                    value={customMax}
                    onChange={e => setCustomMax(e.target.value)}
                    style={{ width: 90, padding: "3px 6px", fontSize: 12, background: "var(--bg)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}
                  />
                </div>
                <div style={{ alignSelf: "flex-end", display: "flex", gap: 8, alignItems: "center" }}>
                  <button
                    className="btn small"
                    onClick={() => act(() => api.pagefileSetCustom(customPath, parseInt(customInit), parseInt(customMax)))}
                    disabled={busy}
                  >
                    Apply
                  </button>
                  <RiskBadge id="pagefile_custom_size" />
                </div>
              </div>
              <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>
                Recommended: Initial = Max = 1.5× RAM ({Math.round(data.ramGb * 1.5 * 1024)} MB) for stability, or 4096–8192 MB for most cases.
              </div>
            </div>
          )}

          {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 10 }}>⚠ {log}</div>}
        </>
      )}
    </Card>
  );
}

// ─── Page ─────────────────────────────────────────────────────────────────────

export default function PerfTweaks({ admin }: { admin: boolean }) {
  return (
    <>
      <div className="page-title">⚡ Performance Tweaks</div>
      <div className="page-sub">
        Advanced system-level tuning — timer resolution, interrupt routing, network latency, memory management.
        All changes are reversible. Admin rights required for most features.
      </div>
      <HwWarnings page="perf_tweaks" />

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 14 }}>
        <TimerCard admin={admin} />
        <RamCard admin={admin} />
      </div>

      <div style={{ marginTop: 14 }}>
        <MsiCard admin={admin} />
      </div>

      <div style={{ marginTop: 14 }}>
        <NetCard admin={admin} />
      </div>

      <div style={{ marginTop: 14 }}>
        <PagefileCard admin={admin} />
      </div>
    </>
  );
}
