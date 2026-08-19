import React, { useEffect, useRef, useState } from "react";
import { api, fmtAge } from "../api";
import { Card, Spinner, RawJson } from "../components/ui";
import type { Mode } from "../App";
import { useLang } from "../i18n";

const ok = (v: any) => v === true || v === "True";

const interp = (tpl: string, params: Record<string, string>) => {
  let s = tpl;
  for (const [k, v] of Object.entries(params)) s = s.replaceAll(`{${k}}`, v);
  return s;
};

/** Collapse long GUID suffixes (e.g. "...TaskMachineCore{3AECA2AC-...}")
 * down to "...TaskMachineCore{…}" so autorun rows don't wrap to 2-3 lines
 * and blow out the card height. Full name is still available via title=. */
function shortenTaskName(name: string): string {
  const collapsed = name.replace(/\{[0-9A-Fa-f-]{8,}\}/g, "{…}");
  return collapsed.length > 46 ? collapsed.slice(0, 44) + "…" : collapsed;
}

function HostsCard({ count, hoDisabledCount }: { count: number; hoDisabledCount: number }) {
  const { t } = useLang();
  const [data, setData] = useState<{ active: string[]; hoDisabled: string[] } | null>(null);
  const [loading, setLoading] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const [tab, setTab] = useState<"active" | "disabled">("active");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);

  const load = async () => {
    setLoading(true);
    setErr("");
    try {
      const d = await api.hostsListAll();
      setData(d);
    } catch (e: any) {
      setErr(String(e));
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
    setErr("");
    try {
      await api.hostsDisableEntries([...selected]);
      clearSel();
      await load();
    } catch (e: any) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const enableSelected = async () => {
    if (!selected.size) return;
    setBusy(true);
    setErr("");
    try {
      await api.hostsEnableEntries([...selected]);
      clearSel();
      await load();
    } catch (e: any) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const title = `🌐 ${t("secHostsTitle")} (${count} ${t("secHostsActiveWord")}${hoDisabledCount > 0 ? `, ${hoDisabledCount} ${t("secHostsDisabledWord")}` : ""})`;

  return (
    <Card title={title} style={{ marginTop: 14 }}>
      {err && <div style={{ color: "var(--red)", fontSize: 12, marginBottom: 8 }}>{err}</div>}
      {!data ? (
        <div className="row" style={{ gap: 10, alignItems: "center" }}>
          <span className="muted" style={{ fontSize: 13 }}>
            {count} {t("secHostsSummary")} {hoDisabledCount > 0 ? `${hoDisabledCount} ${t("secHostsDisabledBy")}` : ""}
          </span>
          <button className="btn small ghost" onClick={load} disabled={loading}>
            {loading ? t("secHostsLoading") : t("secHostsManage")}
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
              {t("secHostsActiveTab")} ({data.active.length})
            </button>
            <button
              className={`btn small ${tab === "disabled" ? "" : "ghost"}`}
              onClick={() => switchTab("disabled")}
            >
              {t("secHostsDisabledTab")} ({data.hoDisabled.length})
            </button>
            <div style={{ flex: 1 }} />
            <button className="btn small ghost" onClick={load} disabled={loading} title={t("secRefresh")}>
              ↺
            </button>
          </div>

          {/* Search + bulk actions */}
          <div className="row" style={{ gap: 6, marginBottom: 8 }}>
            <input
              ref={searchRef}
              className="input"
              placeholder={t("secHostsFilter")}
              value={search}
              onChange={(e) => { setSearch(e.target.value); clearSel(); }}
              style={{ flex: 1, padding: "4px 8px", fontSize: 12, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--text)" }}
            />
            <button className="btn small ghost" onClick={selectAll} title={t("secHostsAll")}>
              {t("secHostsAll")}
            </button>
            <button className="btn small ghost" onClick={clearSel} disabled={!selected.size}>
              {t("secHostsClear")}
            </button>
            {tab === "active" && (
              <button
                className="btn small danger"
                onClick={disableSelected}
                disabled={!selected.size || busy}
              >
                {busy ? "…" : `⛔ ${t("secHostsDisable")} (${selected.size})`}
              </button>
            )}
            {tab === "disabled" && (
              <button
                className="btn small"
                onClick={enableSelected}
                disabled={!selected.size || busy}
              >
                {busy ? "…" : `↩ ${t("secHostsEnable")} (${selected.size})`}
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
                … {filtered.length - 200} {t("secHostsMore")}
              </div>
            )}
            {filtered.length === 0 && (
              <div className="muted" style={{ padding: 10, textAlign: "center" }}>
                {t("secHostsNoEntries")}{search ? t("secHostsMatchingFilter") : ""}
              </div>
            )}
          </div>

          <div className="muted" style={{ fontSize: 11, marginTop: 5 }}>
            {interp(t("secHostsShowing"), { a: String(Math.min(filtered.length, 200)), b: String(filtered.length) })}
            {selected.size > 0 && ` · ${selected.size} ${t("secHostsSelected")}`}
            {tab === "active" && ` · ${t("secHostsReversible")}`}
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
        <h1 className="page-title">{t("secTitle")}</h1>
        <Spinner /> <span className="muted">{t("secInspecting")}</span>
      </>
    );

  const d = sec.defender ?? {};
  const light = (on: boolean) => (
    <span style={{ color: on ? "var(--green)" : "var(--red)", fontWeight: 700 }}>
      {on ? `✓ ${t("secOn")}` : `✗ ${t("secOff")}`}
    </span>
  );

  const fwArr: any[] = Array.isArray(sec.firewall) ? sec.firewall : [];
  const fwMap: Record<string, any> = Object.fromEntries(fwArr.map((f: any) => [f.Name, f]));

  const defenderRows = [
    {
      label: t("secRealTime"),
      on: ok(d.RealTimeProtectionEnabled),
      toggle: (v: boolean) => api.defenderSetRealtime(v).then(() => load(false)),
      canToggle: true,
    },
    {
      label: t("secCloud"),
      on: ok(d.MAPSReporting),
      toggle: (v: boolean) => api.defenderSetCloud(v).then(() => load(false)),
      canToggle: true,
    },
    {
      label: t("secTamper"),
      on: ok(d.IsTamperProtected),
      toggle: null,
      canToggle: false,
    },
    { label: t("secFwDomain"),  on: ok(fwMap["Domain"]?.Enabled),  toggle: null, canToggle: false },
    { label: t("secFwPrivate"), on: ok(fwMap["Private"]?.Enabled), toggle: null, canToggle: false },
    { label: t("secFwPublic"),  on: ok(fwMap["Public"]?.Enabled),  toggle: null, canToggle: false },
  ];

  const susDrivers   = (sec.unsigned_drivers?.items ?? []) as any[];
  const startupItems: any[] = Array.isArray(sec.autoruns?.startup_folder) ? sec.autoruns.startup_folder : [];
  const taskItems: any[]    = Array.isArray(sec.autoruns?.tasks_nonms)     ? sec.autoruns.tasks_nonms     : [];
  const susAutoruns = [...startupItems, ...taskItems];

  return (
    <>
      <h1 className="page-title">🛡 {t("secTitle")}</h1>
      <div className="page-sub">{t("secSub")}</div>

      <div className="row" style={{ gap: 8, marginBottom: 12 }}>
        <button className="btn small" disabled={busy} onClick={() => load(false)}>
          {busy ? <Spinner /> : `⟳ ${t("secRefresh")}`}
        </button>
        <button className="btn small ghost" disabled={busy} onClick={() => load(true)}>
          {busy ? <Spinner /> : `🛡 ${t("secQuickScan")}`}
        </button>
      </div>

      {sec.error && (
        <div style={{ color: "var(--red)", marginBottom: 10, fontSize: 13 }}>⚠ {sec.error}</div>
      )}

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, alignItems: "start" }}>
        {/* Defender + Firewall */}
        <Card title={`🛡️ ${t("secDefenderTitle")}`}>
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
                    catch (e: any) { setTaskMsg({ text: String(e), ok: false }); }
                    finally { setBusy(false); }
                    load(true);
                  }}
                  >
                    {r.on ? t("secDisable") : t("secEnable")}
                  </button>
                )}
              </div>
            </div>
          ))}
        </Card>

        {/* Drivers */}
        <Card title={`⚠️ ${t("secDriversTitle")} (${susDrivers.length})`}>
          {susDrivers.length === 0 ? (
            <span className="muted" style={{ fontSize: 13 }}>✓ {t("secNoSuspiciousDrivers")}</span>
          ) : (
            <>
              <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start", gap: 8, marginBottom: 6 }}>
                <div className="muted" style={{ fontSize: 11 }}>
                  {t("secDriversExplain")}
                </div>
                <button
                  className="btn small ghost"
                  style={{ fontSize: 11, flexShrink: 0, whiteSpace: "nowrap" }}
                  onClick={async () => {
                    try {
                      await api.driversOpenDevmgr();
                      setTaskMsg({ text: t("secDevMgrOpened"), ok: true });
                    } catch (e: any) {
                      setTaskMsg({ text: String(e), ok: false });
                    }
                  }}
                >
                  {t("secOpenDevMgr")}
                </button>
              </div>
              <div style={{ maxHeight: 280, overflowY: "auto" }}>
                {susDrivers.map((raw: any, i: number) => {
                  const dr = typeof raw === "string" ? { device: raw } : raw ?? {};
                  const label = dr.device || dr.name || t("secUnknownDriver");
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
                          {dr.manufacturer || t("secUnknownMfr")}{dr.deviceClass ? ` · ${dr.deviceClass}` : ""}
                        </div>
                      </div>
                      <div className="row" style={{ gap: 6, flexShrink: 0 }}>
                        <button
                          className="btn small ghost"
                          style={{ fontSize: 11 }}
                          disabled={rowBusy || !deviceId}
                          onClick={() => run(api.securityDisableDriver, null, t("secDisabledAction"))}
                        >
                          {rowBusy ? "…" : t("secDisable")}
                        </button>
                        <button
                          className="btn small ghost danger"
                          style={{ fontSize: 11 }}
                          disabled={rowBusy || !deviceId}
                          onClick={() =>
                            run(
                              api.securityRemoveDriver,
                              interp(t("secRemoveConfirm"), { label }),
                              t("secRemovedAction")
                            )
                          }
                        >
                          {rowBusy ? "…" : t("secRemove")}
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
        <Card title={`📋 ${t("secAutorunsTitle")} (${susAutoruns.length})`}>
          {susAutoruns.length === 0 ? (
            <span className="muted" style={{ fontSize: 13 }}>✓ {t("secNoAutoruns")}</span>
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
                      {isDisabled ? t("secDisabled") : t("secEnabled")}
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
                          setTaskMsg({ text: msg || interp(isDisabled ? t("secEnabledAction") : t("secDisabledAction2"), { name: taskName }), ok: true });
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
                      {isDisabled ? t("secEnable") : t("secDisable")}
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
        <Card title={t("secRawJson")} style={{ marginTop: 12 }}>
          <RawJson data={sec} />
        </Card>
      )}
    </>
  );
}
