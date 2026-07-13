import React, { useEffect, useRef, useState } from "react";
import { api, fmtAge } from "../api";
import { Card, Spinner, RawJson } from "../components/ui";
import type { Mode } from "../App";
import { useLang } from "../i18n";

const ok = (v: any) => v === true || v === "True";

/** Collapse long GUID suffixes (e.g. "...TaskMachineCore{3AECA2AC-...}")
 * down to "...TaskMachineCore{…}" so autorun rows don't wrap to 2-3 lines
 * and blow out the card height. Full name is still available via title=. */
function shortenTaskName(name: string): string {
  const collapsed = name.replace(/\{[0-9A-Fa-f-]{8,}\}/g, "{…}");
  return collapsed.length > 46 ? collapsed.slice(0, 44) + "…" : collapsed;
}

function HostsCard({ count, hoDisabledCount }: { count: number; hoDisabledCount: number }) {
  const [data, setData] = useState<{ active: string[]; hoDisabled: string[] } | null>(null);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const [tab, setTab] = useState<"active" | "disabled">("active");
  const [busy, setBusy] = useState(false);
  const searchRef = useRef<HTMLInputElement>(null);

  const load = async () => {
    setLoading(true);
    try {
      const d = await api.hostsListAll();
      setData(d);
    } catch (e: any) {
      alert(String(e));
    } finally {
      setLoading(false);
    }
  };

  const entries = tab === "active" ? (data?.active ?? []) : (data?.hoDisabled ?? []);
  const filtered = search
    ? entries.filter((e) => e.toLowerCase().includes(search.toLowerCase()))
    : entries;

  const toggle = (e: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(e)) next.delete(e);
      else next.add(e);
      return next;
    });

  const selectAll = () => setSelected(new Set(filtered.slice(0, 200)));
  const clearSel = () => setSelected(new Set());

  const switchTab = (t: "active" | "disabled") => {
    setTab(t);
    clearSel();
    setSearch("");
  };

  const disableSelected = async () => {
    if (!selected.size) return;
    setBusy(true);
    try {
      await api.hostsDisableEntries([...selected]);
      clearSel();
      await load();
    } catch (e: any) {
      alert(String(e));
    } finally {
      setBusy(false);
    }
  };

  const enableSelected = async () => {
    if (!selected.size) return;
    setBusy(true);
    try {
      await api.hostsEnableEntries([...selected]);
      clearSel();
      await load();
    } catch (e: any) {
      alert(String(e));
    } finally {
      setBusy(false);
    }
  };

  const title = `🌐 Hosts file entries (${count} active${hoDisabledCount > 0 ? `, ${hoDisabledCount} disabled` : ""})`;

  return (
    <Card title={title} style={{ marginTop: 14 }}>
      {!data ? (
        <div className="row" style={{ gap: 10, alignItems: "center" }}>
          <span className="muted" style={{ fontSize: 13 }}>
            {count} active entries · {hoDisabledCount > 0 ? `${hoDisabledCount} disabled by AD HyperOptimize · ` : ""}
            likely an ad-blocker hosts list.
          </span>
          <button className="btn small ghost" onClick={load} disabled={loading}>
            {loading ? "Loading…" : "Manage entries"}
          </button>
        </div>
      ) : (
        <>
          {/* Tabs */}
          <div className="row" style={{ gap: 6, marginBottom: 10 }}>
            <button
              className={`btn small ${tab === "active" ? "" : "ghost"}`}
              onClick={() => switchTab("active")}
            >
              Active ({data.active.length})
            </button>
            <button
              className={`btn small ${tab === "disabled" ? "" : "ghost"}`}
              onClick={() => switchTab("disabled")}
            >
              Disabled ({data.hoDisabled.length})
            </button>
            <div style={{ flex: 1 }} />
            <button className="btn small ghost" onClick={load} disabled={loading} title="Refresh">
              ↺
            </button>
          </div>

          {/* Search + bulk actions */}
          <div className="row" style={{ gap: 6, marginBottom: 8 }}>
            <input
              ref={searchRef}
              className="input"
              placeholder="Filter entries…"
              value={search}
              onChange={(e) => { setSearch(e.target.value); clearSel(); }}
              style={{ flex: 1, padding: "4px 8px", fontSize: 12, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }}
            />
            <button className="btn small ghost" onClick={selectAll} title="Select all visible">
              All
            </button>
            <button className="btn small ghost" onClick={clearSel} disabled={!selected.size}>
              Clear
            </button>
            {tab === "active" && (
              <button
                className="btn small danger"
                onClick={disableSelected}
                disabled={!selected.size || busy}
              >
                {busy ? "…" : `⛔ Disable (${selected.size})`}
              </button>
            )}
            {tab === "disabled" && (
              <button
                className="btn small"
                onClick={enableSelected}
                disabled={!selected.size || busy}
              >
                {busy ? "…" : `↩ Enable (${selected.size})`}
              </button>
            )}
          </div>

          {/* Entry list */}
          <div
            style={{
              maxHeight: 300,
              overflowY: "auto",
              border: "1px solid var(--border)",
              borderRadius: 4,
              fontSize: 11,
            }}
          >
            {filtered.slice(0, 200).map((entry, i) => (
              <label
                key={i}
                className="row"
                style={{
                  padding: "3px 8px",
                  cursor: "pointer",
                  borderBottom: "1px solid var(--border)",
                  alignItems: "center",
                  gap: 8,
                  opacity: tab === "disabled" ? 0.55 : 1,
                }}
              >
                <input
                  type="checkbox"
                  checked={selected.has(entry)}
                  onChange={() => toggle(entry)}
                  style={{ flexShrink: 0 }}
                />
                <span className="mono" style={{ flex: 1, wordBreak: "break-all" }}>
                  {entry}
                </span>
              </label>
            ))}
            {filtered.length > 200 && (
              <div className="muted" style={{ padding: "4px 8px" }}>
                … {filtered.length - 200} more — refine filter to see them
              </div>
            )}
            {filtered.length === 0 && (
              <div className="muted" style={{ padding: 10, textAlign: "center" }}>
                No {tab} entries{search ? " matching filter" : ""}
              </div>
            )}
          </div>

          <div className="muted" style={{ fontSize: 11, marginTop: 5 }}>
            Showing {Math.min(filtered.length, 200)} / {filtered.length} {tab} entries
            {selected.size > 0 && ` · ${selected.size} selected`}
            {tab === "active" && " · Disabled entries are commented-out in-place (reversible)"}
          </div>
        </>
      )}
    </Card>
  );
}

export default function Security({ mode }: { mode: Mode }) {
  const { t } = useLang();
  const [sec, setSec] = useState<any | null>(null);
  const [meta, setMeta] = useState<{ time: string; fromCache: boolean } | null>(null);
  const [busy, setBusy] = useState(false);
  const [taskMsg, setTaskMsg] = useState<{ text: string; ok: boolean } | null>(null);
  const [driverBusy, setDriverBusy] = useState<string | null>(null);

  const load = (force: boolean) => {
    setBusy(true);
    api
      .securityScan(force)
      .then((env) => {
        setSec(env.data);
        setMeta({ time: env.time, fromCache: env.fromCache });
      })
      .catch((e) => setSec({ error: String(e) }))
      .finally(() => setBusy(false));
  };

  useEffect(() => {
    load(false);
  }, []);

  if (!sec)
    return (
      <>
        <div className="page-title">Security & Anomalies</div>
        <Spinner /> <span className="muted">Inspecting Defender, firewall, drivers, autoruns, hosts…</span>
      </>
    );

  const d = sec.defender ?? {};
  const light = (on: boolean) => (
    <span style={{ color: on ? "var(--green)" : "var(--red)", fontWeight: 700 }}>
      {on ? "✓ On" : "✗ Off"}
    </span>
  );

  const fwArr: any[] = Array.isArray(sec.firewall) ? sec.firewall : [];
  const fwMap: Record<string, any> = Object.fromEntries(fwArr.map((f: any) => [f.Name, f]));

  const defenderRows = [
    {
      label: "Real-Time Protection",
      on: ok(d.RealTimeProtectionEnabled),
      toggle: (v: boolean) => api.defenderSetRealtime(v).then(() => load(false)),
      canToggle: true,
    },
    {
      label: "Cloud Protection",
      on: ok(d.MAPSReporting),
      toggle: (v: boolean) => api.defenderSetCloud(v).then(() => load(false)),
      canToggle: true,
    },
    {
      label: "Tamper Protection",
      on: ok(d.IsTamperProtected),
      toggle: null,
      canToggle: false,
    },
    { label: "Firewall (Domain)",  on: ok(fwMap["Domain"]?.Enabled),  toggle: null, canToggle: false },
    { label: "Firewall (Private)", on: ok(fwMap["Private"]?.Enabled), toggle: null, canToggle: false },
    { label: "Firewall (Public)",  on: ok(fwMap["Public"]?.Enabled),  toggle: null, canToggle: false },
  ];

  const susDrivers   = (sec.unsigned_drivers?.items ?? []) as any[];
  const startupItems: any[] = Array.isArray(sec.autoruns?.startup_folder) ? sec.autoruns.startup_folder : [];
  const taskItems: any[]    = Array.isArray(sec.autoruns?.tasks_nonms)     ? sec.autoruns.tasks_nonms     : [];
  const susAutoruns = [...startupItems, ...taskItems];

  return (
    <>
      <div className="page-title">🛡 Security & Anomalies</div>
      <div className="page-sub">Defender, firewall, unsigned drivers, suspicious autoruns, and hosts file.</div>

      <div className="row" style={{ gap: 8, marginBottom: 12 }}>
        <button className="btn small" disabled={busy} onClick={() => load(false)}>
          {busy ? <Spinner /> : "⟳ Refresh"}
        </button>
        <button className="btn small ghost" disabled={busy} onClick={() => load(true)}>
          {busy ? <Spinner /> : "🛡 Quick Defender Scan"}
        </button>
      </div>

      {sec.error && (
        <div style={{ color: "var(--red)", marginBottom: 10, fontSize: 13 }}>⚠ {sec.error}</div>
      )}

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, alignItems: "start" }}>
        {/* Defender + Firewall */}
        <Card title="🛡️ Windows Defender & Firewall">
          {defenderRows.map(r => (
            <div key={r.label} className="row" style={{ justifyContent: "space-between", padding: "6px 0", borderBottom: "1px solid var(--border)", fontSize: 13, alignItems: "center" }}>
              <span className="muted">{r.label}</span>
              <div className="row" style={{ gap: 8, alignItems: "center" }}>
                {light(r.on)}
                {r.canToggle && (
                  <button
                    className="btn small ghost"
                    style={{ fontSize: 11, padding: "2px 8px", color: r.on ? "var(--red)" : "var(--green)", borderColor: r.on ? "var(--red)" : "var(--green)" }}
                    disabled={busy}
                    onClick={async () => {
                    setBusy(true);
                    try { if (r.toggle) await r.toggle(!r.on); }
                    catch (e: any) { alert(String(e)); }
                    finally { setBusy(false); }
                    load(true);
                  }}
                  >
                    {r.on ? "Disable" : "Enable"}
                  </button>
                )}
              </div>
            </div>
          ))}
        </Card>

        {/* Drivers */}
        <Card title={`⚠️ Unsigned / Suspicious Drivers (${susDrivers.length})`}>
          {susDrivers.length === 0 ? (
            <span className="muted" style={{ fontSize: 13 }}>✓ No suspicious drivers found.</span>
          ) : (
            <>
              <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start", gap: 8, marginBottom: 6 }}>
                <div className="muted" style={{ fontSize: 11 }}>
                  Disable is reversible (driver stays installed, just stops loading). Remove uninstalls it —
                  if the hardware is still plugged in, Windows usually redetects and reinstalls a driver on its own
                  (the same "repair" trick as Device Manager's own Uninstall button). Consider a restore point first.
                </div>
                <button
                  className="btn small ghost"
                  style={{ fontSize: 11, flexShrink: 0, whiteSpace: "nowrap" }}
                  onClick={async () => {
                    try {
                      await api.driversOpenDevmgr();
                      setTaskMsg({ text: "Device Manager opened.", ok: true });
                    } catch (e: any) {
                      setTaskMsg({ text: String(e), ok: false });
                    }
                  }}
                >
                  Open Device Manager
                </button>
              </div>
              <div style={{ maxHeight: 280, overflowY: "auto" }}>
                {susDrivers.map((raw: any, i: number) => {
                  const dr = typeof raw === "string" ? { device: raw } : raw ?? {};
                  const label = dr.device || dr.name || "Unknown driver";
                  const deviceId: string = dr.deviceId || "";
                  const rowBusy = driverBusy === deviceId;
                  const run = async (
                    action: (id: string) => Promise<string>,
                    confirmMsg: string | null,
                    busyLabel: string
                  ) => {
                    if (!deviceId) {
                      setTaskMsg({ text: `No device instance ID for "${label}" — open Device Manager instead.`, ok: false });
                      return;
                    }
                    if (confirmMsg && !window.confirm(confirmMsg)) return;
                    setDriverBusy(deviceId);
                    try {
                      const msg = await action(deviceId);
                      setTaskMsg({ text: `${busyLabel} "${label}": ${msg}`, ok: true });
                      load(true);
                    } catch (e: any) {
                      setTaskMsg({ text: String(e), ok: false });
                    } finally {
                      setDriverBusy(null);
                    }
                  };
                  return (
                    <div key={deviceId || i} className="row" style={{ fontSize: 12, padding: "6px 0", borderBottom: "1px solid var(--border)", gap: 8, alignItems: "center" }}>
                      <div style={{ flex: 1, minWidth: 0 }} title={deviceId || label}>
                        <div style={{ fontWeight: 600, color: "var(--orange)", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                          {label}
                        </div>
                        <div className="muted" style={{ fontSize: 11, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                          {dr.manufacturer || "Unknown manufacturer"}{dr.deviceClass ? ` · ${dr.deviceClass}` : ""}
                        </div>
                      </div>
                      <div className="row" style={{ gap: 6, flexShrink: 0 }}>
                        <button
                          className="btn small ghost"
                          style={{ fontSize: 11 }}
                          disabled={rowBusy || !deviceId}
                          onClick={() => run(api.securityDisableDriver, null, "Disabled")}
                        >
                          {rowBusy ? "…" : "Disable"}
                        </button>
                        <button
                          className="btn small ghost danger"
                          style={{ fontSize: 11 }}
                          disabled={rowBusy || !deviceId}
                          onClick={() =>
                            run(
                              api.securityRemoveDriver,
                              `Remove "${label}"?\n\nThis uninstalls the driver. If the hardware is still connected, Windows will likely reinstall a driver for it automatically. A restore point is recommended first.`,
                              "Removed"
                            )
                          }
                        >
                          {rowBusy ? "…" : "Remove"}
                        </button>
                      </div>
                    </div>
                  );
                })}
              </div>
            </>
          )}
        </Card>
      </div>

      {taskMsg && (
        <div style={{ marginTop: 12, padding: "8px 12px", borderRadius: 6, fontSize: 13,
          background: taskMsg.ok ? "rgba(80,200,120,0.08)" : "rgba(255,80,80,0.08)",
          border: `1px solid ${taskMsg.ok ? "var(--green)" : "var(--red)"}`,
          color: taskMsg.ok ? "var(--green)" : "var(--red)",
          display: "flex", gap: 10, alignItems: "center" }}>
          <span>{taskMsg.ok ? "✓" : "✗"}</span>
          <span style={{ flex: 1 }}>{taskMsg.text}</span>
          <button onClick={() => setTaskMsg(null)} style={{ background: "none", border: "none", cursor: "pointer", color: "inherit", fontSize: 16 }}>×</button>
        </div>
      )}

      <div style={{ marginTop: 12, display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, alignItems: "start" }}>
        {/* Autoruns */}
        <Card title={`📋 Autoruns / Scheduled Tasks (${susAutoruns.length})`}>
          {susAutoruns.length === 0 ? (
            <span className="muted" style={{ fontSize: 13 }}>✓ No non-Microsoft startup tasks found.</span>
          ) : (
            <div style={{ maxHeight: 360, overflowY: "auto" }}>
              {susAutoruns.map((a: any, i: number) => {
                const taskName = a.TaskName ?? a.Name ?? a.name ?? "";
                const taskPath = a.TaskPath ?? "\\";
                const isDisabled = a.State === "Disabled";
                return (
                  <div key={i} className="row" style={{ padding: "6px 0", borderBottom: "1px solid var(--border)", gap: 8, alignItems: "center" }}>
                    <div style={{ flex: 1, minWidth: 0 }} title={taskName}>
                      <div style={{ fontWeight: 600, fontSize: 12, color: isDisabled ? "var(--muted)" : undefined, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
                        {shortenTaskName(taskName)}
                      </div>
                      <div className="muted" style={{ fontSize: 11, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>{taskPath}</div>
                    </div>
                    <span style={{ fontSize: 10, color: isDisabled ? "var(--muted)" : "var(--green)", flexShrink: 0 }}>
                      {isDisabled ? "Disabled" : "Enabled"}
                    </span>
                    <button
                      className="btn small ghost"
                      style={{ fontSize: 11, flexShrink: 0, color: isDisabled ? "var(--green)" : "var(--red)", borderColor: isDisabled ? "var(--green)" : "var(--red)" }}
                      disabled={busy}
                      onClick={async () => {
                        setBusy(true);
                        setTaskMsg(null);
                        try {
                          const msg = isDisabled
                            ? await api.enableScheduledTask(taskPath, taskName)
                            : await api.disableScheduledTask(taskPath, taskName);
                          setTaskMsg({ text: msg || (isDisabled ? `Enabled: ${taskName}` : `Disabled: ${taskName}`), ok: true });
                          // optimistic UI flip
                          setSec((prev: any) => {
                            if (!prev?.autoruns?.tasks_nonms) return prev;
                            return {
                              ...prev,
                              autoruns: {
                                ...prev.autoruns,
                                tasks_nonms: prev.autoruns.tasks_nonms.map((t: any) =>
                                  t.TaskName === taskName && t.TaskPath === taskPath
                                    ? { ...t, State: isDisabled ? "Ready" : "Disabled" }
                                    : t
                                ),
                              },
                            };
                          });
                        } catch (e: any) {
                          setTaskMsg({ text: String(e), ok: false });
                        } finally {
                          setBusy(false);
                        }
                      }}
                    >
                      {isDisabled ? "Enable" : "Disable"}
                    </button>
                  </div>
                );
              })}
            </div>
          )}
        </Card>

        {/* Hosts file */}
        <HostsCard
          count={sec.hosts?.active?.length ?? 0}
          hoDisabledCount={sec.hosts?.disabled?.length ?? 0}
        />
      </div>

      {mode === "expert" && sec && (
        <Card title="Raw JSON" style={{ marginTop: 12 }}>
          <RawJson data={sec} />
        </Card>
      )}
    </>
  );
}
