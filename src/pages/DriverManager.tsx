import React, { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";

type Driver = {
  DeviceName: string;
  DeviceClass: string;
  DriverVersion: string;
  Manufacturer: string;
  IsSigned: boolean;
  dateStr: string;
  old: boolean;
  inbox: boolean;
  wingetId: string | null;
  vendorUrl: string;
  vendorLabel: string;
};

type WingetStatus = { available: boolean; current?: string; newVersion?: string; error?: string };

function DriverRow({ d, admin }: { d: Driver; admin?: boolean }) {
  const [open, setOpen]           = useState(false);
  const [wgStatus, setWgStatus]   = useState<WingetStatus | null>(null);
  const [wgChecking, setWgChecking] = useState(false);
  const [busy, setBusy]           = useState(false);
  const [log, setLog]             = useState("");

  const checkWinget = async () => {
    if (!d.wingetId) return;
    setWgChecking(true);
    try {
      const r = await api.driversCheckWinget(d.wingetId);
      setWgStatus(r);
    } catch (e: any) {
      setWgStatus({ available: false, error: String(e) });
    } finally { setWgChecking(false); }
  };

  const installWinget = async () => {
    if (!d.wingetId) return;
    setBusy(true);
    setLog("Installing via winget — this may take several minutes…");
    try {
      const r = await api.driversInstallWinget(d.wingetId);
      setLog(r);
      setWgStatus(null); // reset so user can re-check
    } catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  const openVendor = async () => {
    try { await api.driversOpenVendorUrl(d.vendorUrl); }
    catch (e: any) { setLog(String(e)); }
  };

  const updateViaWU = async () => {
    setBusy(true);
    setLog("Triggering Windows Update driver scan…");
    try { setLog(await api.driversScanWindowsUpdate()); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  return (
    <div style={{ borderBottom: "1px solid var(--border)" }}>
      {/* Main row — clickable to expand */}
      <div
        className="row"
        style={{ padding: "7px 0", gap: 8, cursor: "pointer", alignItems: "flex-start" }}
        onClick={() => setOpen(o => !o)}
      >
        <span style={{ fontSize: 11, color: "var(--muted)", minWidth: 110, flexShrink: 0, paddingTop: 2 }}>
          {d.DeviceClass}
        </span>
        <div style={{ flex: 1 }}>
          <div className="row" style={{ gap: 6, alignItems: "center", flexWrap: "wrap" }}>
            <b style={{ fontSize: 13 }}>{d.DeviceName}</b>
            {d.old && !d.inbox && (
              <span style={{ fontSize: 10, padding: "1px 6px", borderRadius: 4, background: "var(--red)", color: "#fff", fontWeight: 700 }}>OLD</span>
            )}
            {!d.IsSigned && (
              <span style={{ fontSize: 10, padding: "1px 6px", borderRadius: 4, background: "var(--orange)", color: "#fff", fontWeight: 700 }}>UNSIGNED</span>
            )}
            {wgStatus?.available && (
              <span style={{ fontSize: 10, padding: "1px 6px", borderRadius: 4, background: "var(--green)", color: "#fff", fontWeight: 700 }}>UPDATE AVAILABLE</span>
            )}
          </div>
          <div className="muted" style={{ fontSize: 11 }}>
            {d.Manufacturer && <span>{d.Manufacturer} · </span>}
            v{d.DriverVersion}
            {d.dateStr && <span> · {d.dateStr}</span>}
          </div>
        </div>
        <span style={{ fontSize: 13, color: "var(--muted)", paddingTop: 2 }}>{open ? "▲" : "▼"}</span>
      </div>

      {/* Expanded update panel */}
      {open && (
        <div style={{ padding: "10px 12px 12px", background: "var(--bg2)", borderRadius: 6, marginBottom: 8, marginTop: -2 }}>
          <div className="muted" style={{ fontSize: 11, marginBottom: 10 }}>
            <b>Class:</b> {d.DeviceClass} &nbsp;·&nbsp;
            <b>Manufacturer:</b> {d.Manufacturer} &nbsp;·&nbsp;
            <b>Version:</b> {d.DriverVersion} &nbsp;·&nbsp;
            <b>Date:</b> {d.dateStr || "unknown"} &nbsp;·&nbsp;
            <b>Signed:</b> {d.IsSigned ? "Yes" : "No"}
          </div>

          <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
            {/* winget */}
            {d.wingetId && (
              wgStatus?.available ? (
                <button className="btn small" disabled={busy} onClick={installWinget}>
                  {busy ? <><Spinner /> Installing…</> : `⬆ Install via winget${wgStatus.newVersion ? ` (${wgStatus.newVersion})` : ""}`}
                </button>
              ) : (
                <button className="btn small ghost" disabled={wgChecking || busy} onClick={checkWinget}>
                  {wgChecking ? <><Spinner /> Checking…</> : "🔍 Check winget"}
                </button>
              )
            )}
            {wgStatus && !wgStatus.available && !wgStatus.error && (
              <span className="muted" style={{ fontSize: 12, alignSelf: "center" }}>winget: up to date</span>
            )}

            {/* vendor download page */}
            <button className="btn small ghost" onClick={openVendor}>
              🌐 {d.vendorLabel}
            </button>

            {/* Windows Update */}
            <button className="btn small ghost" disabled={busy} onClick={updateViaWU}>
              🪟 Windows Update scan
            </button>

            {/* Device Manager */}
            <button className="btn small ghost" onClick={() => api.driversOpenDevmgr()}>
              ⚙ Device Manager
            </button>
          </div>

          {wgStatus?.error && (
            <div className="muted" style={{ fontSize: 11, marginTop: 6 }}>winget: {wgStatus.error}</div>
          )}
          {log && (
            <div className="mono muted" style={{ fontSize: 11, marginTop: 8, whiteSpace: "pre-wrap", maxHeight: 120, overflowY: "auto" }}>
              {log}
            </div>
          )}

          {d.wingetId && (
            <div className="muted" style={{ fontSize: 10, marginTop: 8 }}>
              winget ID: <span className="mono">{d.wingetId}</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

export default function DriverManager() {
  const [data, setData]       = useState<any>(null);
  const [loading, setLoading] = useState(true);
  const [search, setSearch]   = useState("");
  const [filter, setFilter]   = useState<"all" | "old" | "unsigned">("all");
  const [log, setLog]         = useState("");

  const load = async () => {
    setLoading(true);
    try { setData(await api.driversList()); }
    finally { setLoading(false); }
  };

  useEffect(() => { load(); }, []);

  const drivers: Driver[] = data?.drivers ?? [];

  const filtered = useMemo(() => {
    let list = drivers;
    if (filter === "old")      list = list.filter(d => d.old && !d.inbox);
    if (filter === "unsigned") list = list.filter(d => !d.IsSigned);
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter(d =>
        d.DeviceName?.toLowerCase().includes(q) ||
        d.Manufacturer?.toLowerCase().includes(q) ||
        d.DeviceClass?.toLowerCase().includes(q)
      );
    }
    return list;
  }, [drivers, filter, search]);

  const oldCount      = drivers.filter(d => d.old && !d.inbox).length;
  const unsignedCount = drivers.filter(d => !d.IsSigned).length;

  return (
    <>
      <div className="page-title">🔧 Driver Manager</div>
      <div className="page-sub">
        Click any driver to see update options — winget, vendor page, or Windows Update.
        Inbox/system drivers (managed by Windows) are excluded from the OLD flag.
      </div>

      <Card title="">
        <div className="row" style={{ gap: 8, marginBottom: 12, flexWrap: "wrap" }}>
          <button className="btn small ghost" onClick={() => api.driversOpenWindowsUpdate().catch(() => {})}>
            🪟 Open Windows Update
          </button>
          <button className="btn small ghost" onClick={() => api.driversOpenDevmgr().catch(() => {})}>
            ⚙ Device Manager
          </button>
          <button className="btn small ghost" onClick={load} disabled={loading}>↺ Refresh</button>
        </div>

        {loading ? (
          <div className="row" style={{ gap: 10 }}>
            <Spinner /><span className="muted">Scanning drivers — may take 15–30s…</span>
          </div>
        ) : (
          <>
            <div className="row" style={{ gap: 8, marginBottom: 10, flexWrap: "wrap" }}>
              <input
                placeholder="Search name, class, manufacturer…"
                value={search}
                onChange={e => setSearch(e.target.value)}
                style={{ flex: 1, minWidth: 160, padding: "5px 10px", fontSize: 13, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}
              />
              {(["all", "old", "unsigned"] as const).map(f => (
                <button key={f} className={`btn small ${filter === f ? "" : "ghost"}`} onClick={() => setFilter(f)}>
                  {f === "all" ? `All (${drivers.length})`
                    : f === "old" ? `⚠ Old (${oldCount})`
                    : `⚠ Unsigned (${unsignedCount})`}
                </button>
              ))}
            </div>

            <div style={{ maxHeight: "60vh", overflowY: "auto" }}>
              {filtered.length === 0 ? (
                <div className="muted">No drivers match filter.</div>
              ) : filtered.map((d, i) => (
                <DriverRow key={`${d.DeviceName}-${i}`} d={d} />
              ))}
            </div>

            <div className="muted" style={{ fontSize: 12, marginTop: 10 }}>
              Click a driver row to expand update options. winget covers NVIDIA, AMD, Intel, Realtek.
              For everything else, use Windows Update or the vendor page.
            </div>
          </>
        )}
      </Card>
    </>
  );
}
