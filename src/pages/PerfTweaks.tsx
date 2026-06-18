import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { HwWarnings, RiskBadge, RiskNotice } from "../components/HwWarnings";
import { useHwProfile } from "../hooks/useHwProfile";
import { useLang } from "../i18n";

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
  const { t } = useLang();
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
    <Card title={t("perfTimerTitle")}>
      <div className="muted" style={{ fontSize: 12, marginBottom: 4 }}>
        {t("perfTimerDesc")}
      </div>
      <div style={{ marginBottom: 10 }}><RiskBadge id="timer_resolution_min" /></div>
      {!data ? <Spinner /> : data.error ? (
        <div className="muted">{data.error}</div>
      ) : (
        <>
          <table className="tbl" style={{ marginBottom: 10 }}><tbody>
            <tr>
              <td className="muted">{t("perfCurrent")}</td>
              <td><span style={{ color: currentColor, fontWeight: 700 }}>
                {data.currentMs != null ? `${data.currentMs} ms` : "—"}
              </span></td>
            </tr>
            <tr><td className="muted">{t("perfTimerMinPossible")}</td><td>{data.minMs != null ? `${data.minMs} ms` : "—"}</td></tr>
            <tr><td className="muted">{t("perfTimerMaxDefault")}</td><td>{data.maxMs != null ? `${data.maxMs} ms` : "—"}</td></tr>
            <tr>
              <td className="muted">{t("perfTimerPersistent")}</td>
              <td>
                <span style={{ color: data.persistent ? "var(--green)" : data.globalReqEnabled ? "var(--accent)" : "var(--red)" }}>
                  {data.persistent
                    ? `✓ ${t("perfTimerActiveBg")}`
                    : data.globalReqEnabled
                      ? `~ ${t("perfTimerConfiguredReboot")}`
                      : `✗ ${t("perfNotSet")}`}
                </span>
              </td>
            </tr>
          </tbody></table>
          {!admin && <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>⚠ {t("perfAdminRequiredApply")}</div>}
          <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
            {minNeedsAck ? (
              <button className="btn small" style={{ background: "var(--red)", borderColor: "var(--red)" }} disabled={busy || !admin} onClick={() => setRiskAck(true)}>
                ⚠ {t("perfTimerReviewRisk")}
              </button>
            ) : (
              <button className="btn small" onClick={() => apply(5000)} disabled={busy || !admin}>
                ⚡ {t("perfTimerSetGaming")}
              </button>
            )}
            <button className="btn small ghost" onClick={() => apply(10000)} disabled={busy || !admin}>
              {t("perfTimerSet1ms")}
            </button>
            <button className="btn small ghost" onClick={reset} disabled={busy || !admin}>
              ↩ {t("perfResetDefault")}
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
  const { t } = useLang();
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
    <Card title={t("perfMsiTitle")}>
      <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>
        {t("perfMsiDesc")}
      </div>
      {!data ? <Spinner /> : (
        <>
          {devices.length === 0 && <div className="muted">{t("perfMsiNoDevices")}</div>}
          {devices.map((d: any, i: number) => (
            <div key={i} className="row" style={{ padding: "6px 0", borderBottom: "1px solid var(--border)", alignItems: "center", gap: 10 }}>
              <span style={{ color: "var(--muted)", fontSize: 11, width: 36, flexShrink: 0 }}>[{d.type}]</span>
              <span style={{ flex: 1 }}>{d.name}</span>
              <span style={{ color: d.msiEnabled ? "var(--green)" : "var(--red)", fontWeight: 700, fontSize: 12 }}>
                {d.msiEnabled ? `● ${t("perfMsiOn")}` : `● ${t("perfMsiLine")}`}
              </span>
              <RiskBadge id="msi_mode_toggle" />
              <button
                className={`btn small ${d.msiEnabled ? "ghost" : ""}`}
                onClick={() => toggle(d)}
                disabled={busy === d.regPath || !admin}
              >
                {busy === d.regPath ? "…" : d.msiEnabled ? t("perfDisable") : t("perfMsiEnable")}
              </button>
            </div>
          ))}
          {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 8 }}>⚠ {log}</div>}
          <div className="muted" style={{ fontSize: 11, marginTop: 8 }}>
            {t("perfMsiRebootNote")}
          </div>
        </>
      )}
    </Card>
  );
}

// ─── Network Adapter Tweaks ────────────────────────────────────────────────────

function NetCard({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData] = useState<any>(null);
  const [busy, setBusy] = useState(false);
  const [log, setLog] = useState("");

  const NET_PROPS: { key: string; label: string; keyword: string; offVal: number; onVal: number }[] = [
    { key: "intMod",     label: t("perfNetIntMod"),     keyword: "*InterruptModeration",      offVal: 0, onVal: 1 },
    { key: "rss",        label: t("perfNetRss"),        keyword: "*RSS",                       offVal: 0, onVal: 1 },
    { key: "tcpOffload", label: t("perfNetTcpOffload"), keyword: "*TCPChecksumOffloadIPv4",   offVal: 0, onVal: 1 },
    { key: "lsoV2",      label: t("perfNetLsoV2"),      keyword: "*LsoV2IPv4",                offVal: 0, onVal: 1 },
  ];

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
    if (!window.confirm(t("perfNetResetConfirm"))) return;
    setBusy(true);
    try { setLog(await api.netResetAll()); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  const adapters: any[] = data?.adapters ?? [];

  const isDisabled = (val: string | null | undefined) =>
    val == null || val === "" || val === "0" || val?.toLowerCase().includes("disab");

  return (
    <Card title={t("perfNetTitle")}>
      <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>
        {t("perfNetDesc")}
      </div>
      {!data ? <Spinner /> : (
        <>
          {adapters.length === 0 && <div className="muted">{t("perfNetNoAdapters")}</div>}
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
                      {off ? t("perfEnable") : t("perfDisable")}
                    </button>
                  </div>
                );
              })}
            </div>
          ))}
          {!admin && <div className="muted" style={{ fontSize: 12 }}>⚠ {t("perfAdminRequired")}</div>}
          <div className="row" style={{ gap: 8, marginTop: 10 }}>
            <button className="btn small" onClick={allGaming} disabled={busy || !admin}>
              ⚡ {t("perfNetDisableAllGaming")}
            </button>
            <button className="btn small ghost" onClick={resetAll} disabled={busy || !admin}>
              ↩ {t("perfNetResetAllDefaults")}
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
  const { t } = useLang();
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
    <Card title={t("perfRamTitle")}>
      <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>
        {t("perfRamDesc")}
      </div>
      {!data ? <Spinner /> : data.error ? (
        <div className="muted">{data.error}</div>
      ) : (
        <>
          <table className="tbl" style={{ marginBottom: 10 }}><tbody>
            <tr><td className="muted">{t("perfRamTotal")}</td><td>{mb(data.totalMb)}</td></tr>
            <tr>
              <td className="muted">{t("perfRamInUse")}</td>
              <td>
                {mb(data.usedMb)} ({usedPct}%)
                <TempBar pct={usedPct} color={usedPct > 85 ? "var(--red)" : "var(--accent)"} />
              </td>
            </tr>
            <tr>
              <td className="muted">{t("perfRamStandbyCache")}</td>
              <td>
                {mb(data.standbyMb)} ({standbyPct}%)
                <TempBar pct={standbyPct} color="var(--muted)" />
              </td>
            </tr>
            <tr><td className="muted">{t("perfRamModifiedList")}</td><td>{mb(data.modifiedMb)}</td></tr>
            <tr><td className="muted">{t("perfRamFree")}</td><td style={{ color: "var(--green)" }}>{mb(data.freeMb)}</td></tr>
          </tbody></table>
          <div className="row" style={{ gap: 8, alignItems: "center" }}>
            <button className="btn small" onClick={flush} disabled={busy || !admin}>
              {busy ? <><Spinner /> {t("perfRamFlushing")}</> : `⚡ ${t("perfRamFlushStandby")}`}
            </button>
            <button className="btn small ghost" onClick={load} disabled={busy}>↺ {t("perfRefresh")}</button>
            <RiskBadge id="ram_flush_standby" />
          </div>
          {!admin && <div className="muted" style={{ fontSize: 12, marginTop: 6 }}>⚠ {t("perfAdminRequired")}</div>}
          {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 8 }}>{log}</div>}
        </>
      )}
    </Card>
  );
}

// ─── Pagefile ─────────────────────────────────────────────────────────────────

function PagefileCard({ admin }: { admin: boolean }) {
  const { t } = useLang();
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
    <Card title={t("perfPagefileTitle")}>
      <div className="muted" style={{ fontSize: 12, marginBottom: 10 }}>
        {t("perfPagefileDesc")}
      </div>
      {!data ? <Spinner /> : data.error ? (
        <div className="muted">{data.error}</div>
      ) : (
        <>
          <table className="tbl" style={{ marginBottom: 10 }}><tbody>
            <tr><td className="muted">{t("perfPagefileRamInstalled")}</td><td>{data.ramGb} GB</td></tr>
            <tr>
              <td className="muted">{t("perfPagefileMode")}</td>
              <td style={{ color: data.autoManaged ? "var(--accent)" : "var(--fg)" }}>
                {data.autoManaged ? t("perfPagefileAutoManaged") : t("perfPagefileCustomDisabled")}
              </td>
            </tr>
            {files.map((f: any, i: number) => (
              <React.Fragment key={i}>
                <tr><td className="muted">{t("perfPagefilePath")}</td><td className="mono">{f.path}</td></tr>
                <tr><td className="muted">{t("perfPagefileSize")}</td><td>{f.initialMb}–{f.maxMb} MB ({t("perfPagefilePeak")}: {f.peakMb} MB)</td></tr>
              </React.Fragment>
            ))}
            {files.length === 0 && !data.autoManaged && (
              <tr><td className="muted" colSpan={2}>{t("perfPagefileNone")}</td></tr>
            )}
          </tbody></table>

          {!admin && <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>⚠ {t("perfAdminRequired")}</div>}

          <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
            <button className="btn small ghost" onClick={() => act(api.pagefileSetAuto)} disabled={busy || !admin}>
              ↩ {t("perfPagefileAutoBtn")}
            </button>
            <button className="btn small ghost" onClick={() => setShowCustom(s => !s)} disabled={!admin}>
              ✎ {t("perfPagefileCustomSize")}
            </button>
            <button
              className="btn small danger"
              onClick={() => {
                if (pfRisk && !window.confirm(`${pfRisk.title}\n\n${pfRisk.message}\n\n${t("perfPagefileContinueAnyway")}`)) return;
                if (!pfRisk && data.ramGb < 28 && !window.confirm(`${t("perfPagefileLowRamWarnPrefix")} ${data.ramGb} GB ${t("perfPagefileLowRamWarnSuffix")}`)) return;
                act(api.pagefileDisable);
              }}
              disabled={busy || !admin}
            >
              ⛔ {t("perfPagefileDisableBtn")}
            </button>
          </div>
          <div style={{ marginTop: 6 }}><RiskBadge id="pagefile_disable" /></div>
          {pfRisk && <RiskNotice id="pagefile_disable" />}

          {showCustom && (
            <div style={{ marginTop: 12, padding: 10, background: "var(--bg2)", borderRadius: 6, border: "1px solid var(--border)" }}>
              <div className="row" style={{ gap: 8, flexWrap: "wrap", alignItems: "center" }}>
                <div>
                  <div className="muted" style={{ fontSize: 11 }}>{t("perfPagefilePath")}</div>
                  <input
                    className="input"
                    value={customPath}
                    onChange={e => setCustomPath(e.target.value)}
                    style={{ width: 160, padding: "3px 6px", fontSize: 12, background: "var(--bg)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}
                  />
                </div>
                <div>
                  <div className="muted" style={{ fontSize: 11 }}>{t("perfPagefileInitialMb")}</div>
                  <input
                    className="input" type="number"
                    value={customInit}
                    onChange={e => setCustomInit(e.target.value)}
                    style={{ width: 90, padding: "3px 6px", fontSize: 12, background: "var(--bg)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}
                  />
                </div>
                <div>
                  <div className="muted" style={{ fontSize: 11 }}>{t("perfPagefileMaxMb")}</div>
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
                    {t("perfPagefileApply")}
                  </button>
                  <RiskBadge id="pagefile_custom_size" />
                </div>
              </div>
              <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>
                {t("perfPagefileRecPrefix")} ({Math.round(data.ramGb * 1.5 * 1024)} MB) {t("perfPagefileRecSuffix")}
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
  const { t } = useLang();
  return (
    <>
      <div className="page-title">⚡ {t("perfPageTitle")}</div>
      <div className="page-sub">
        {t("perfPageSub")}
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
