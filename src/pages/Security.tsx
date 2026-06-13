import React, { useEffect, useRef, useState } from "react";
import { api, fmtAge } from "../api";
import { Card, Spinner, RawJson } from "../components/ui";
import type { Mode } from "../App";
import { useLang } from "../i18n";

const ok = (v: any) => v === true || v === "True";

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

  const title = `Hosts file entries (${count} active${hoDisabledCount > 0 ? `, ${hoDisabledCount} disabled` : ""})`;

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
    <span style={{ color: on ? "var(--green)" : "var(--red)", fontWeight: 700 }}>{on ? "● ON" : "● OFF"}</span>
  );

  const arr = (v: any) => (Array.isArray(v) ? v : v && !v.error ? [v] : []);

  return (
    <>
      <div className="page-title">Security & Anomalies</div>
      <div className="page-sub">
        Read-only inspection — this page never removes anything itself.
        {meta && <> · Scanned {fmtAge(meta.time)}{meta.fromCache ? " (cached)" : ""} · </>}
        <a style={{ color: "var(--accent)", cursor: "pointer" }} onClick={() => load(true)}>
          {busy ? "rescanning…" : "Re-scan now"}
        </a>
      </div>

      <div className="grid grid-3">
        <Card title="Microsoft Defender">
          <table className="tbl"><tbody>
            <tr><td className="muted">Real-time protection</td><td>{light(ok(d.RealTimeProtectionEnabled))}</td></tr>
            <tr><td className="muted">Antivirus engine</td><td>{light(ok(d.AntivirusEnabled))}</td></tr>
            <tr><td className="muted">Tamper protection</td><td>{light(ok(d.IsTamperProtected))}</td></tr>
          </tbody></table>
        </Card>
        <Card title="Firewall profiles">
          <table className="tbl"><tbody>
            {arr(sec.firewall).map((p: any, i: number) => (
              <tr key={i}><td className="muted">{p.Name}</td><td>{light(p.Enabled === true || p.Enabled === 1)}</td></tr>
            ))}
          </tbody></table>
        </Card>
        <Card title="Platform">
          <table className="tbl"><tbody>
            <tr><td className="muted">Secure Boot</td><td>{String(sec.secure_boot)}</td></tr>
            <tr><td className="muted">UAC</td><td>{light(sec.uac?.EnableLUA === 1)}</td></tr>
          </tbody></table>
        </Card>
      </div>

      <Card title={`Unsigned drivers (${sec.unsigned_drivers?.count ?? "?"})`} style={{ marginTop: 14 }}>
        {sec.unsigned_drivers?.count === 0 && <div className="muted">All loaded drivers are signed. ✔</div>}
        {arr(sec.unsigned_drivers?.items).slice(0, 25).map((u: any, i: number) => (
          <div key={i} className="row" style={{ padding: "4px 0", borderBottom: "1px solid var(--border)" }}>
            <span className="mono" style={{ flex: 1 }}>
              {u.device}
              {u.count > 1 && <span className="muted"> ×{u.count}</span>}
              {u.manufacturer && <span className="muted" style={{ fontSize: 11 }}> · {u.manufacturer}</span>}
            </span>
            <button
              className="btn small ghost"
              onClick={() => api.openPath(`https://www.google.com/search?q=${encodeURIComponent(`"${u.device}" driver unsigned`)}`)}
            >
              {t("secCheckOnline")}
            </button>
          </div>
        ))}
        <div className="muted mt" style={{ fontSize: 12 }}>
          {t("secLighspeedNote")}
        </div>
      </Card>

      <Card title={`Processes running from Temp (${arr(sec.suspicious_processes).length})`} style={{ marginTop: 14 }}>
        {arr(sec.suspicious_processes).length === 0 ? (
          <div className="muted">None — good. Legitimate software rarely executes from Temp.</div>
        ) : (
          <>
            {arr(sec.suspicious_processes).map((p: any, i: number) => (
              <div key={i} className="row" style={{ padding: "4px 0", borderBottom: "1px solid var(--border)" }}>
                <span className="mono" style={{ color: "var(--orange)", flex: 1, wordBreak: "break-all" }}>
                  PID {p.ProcessId} · {p.Name} · {p.ExecutablePath}
                </span>
                <button
                  className="btn small ghost"
                  onClick={() => api.openPath(String(p.ExecutablePath).replace(/\\[^\\]+$/, ""))}
                >
                  {t("secFolder")}
                </button>
                <button
                  className="btn small ghost"
                  onClick={() => api.openPath(`https://www.google.com/search?q=${encodeURIComponent(`"${p.Name}" temp folder malware`)}`)}
                >
                  {t("secCheckOnline")}
                </button>
                <button
                  className="btn small danger"
                  onClick={async () => {
                    if (!window.confirm(`${p.Name} (PID ${p.ProcessId}) ${t("secKillConfirm")}`)) return;
                    try {
                      await api.procKill(p.ProcessId);
                      load(true);
                    } catch (e: any) {
                      alert(String(e));
                    }
                  }}
                >
                  {t("secKill")}
                </button>
              </div>
            ))}
            <div className="muted mt" style={{ fontSize: 12 }}>
              {t("secInstallerNote")}
            </div>
          </>
        )}
        <div className="row mt">
          <button
            className="btn small ghost"
            onClick={async () => alert(await api.defenderQuickScan())}
          >
            {t("secQuickScan")}
          </button>
        </div>
      </Card>

      <HostsCard
        count={sec.hosts?.count ?? 0}
        hoDisabledCount={sec.hosts?.hoDisabledCount ?? 0}
      />

      <Card title="Autorun / persistence locations" style={{ marginTop: 14 }}>
        {(["hklm_run", "hkcu_run"] as const).map((k) => {
          const entries = sec.autoruns?.[k];
          const items = entries && !entries.error ? Object.entries(entries) : [];
          return (
            <div key={k} className="mt">
              <b className="muted">{k === "hklm_run" ? "HKLM Run (all users)" : "HKCU Run (this user)"}</b>
              {items.length === 0 && <div className="muted">empty</div>}
              {items.map(([name, cmd], i) => (
                <div key={i} className="mono muted">{name} → {String(cmd)}</div>
              ))}
            </div>
          );
        })}
        <div className="mt">
          <b className="muted">Non-Microsoft scheduled tasks</b>
          {arr(sec.autoruns?.tasks_nonms).slice(0, 20).map((t: any, i: number) => (
            <div key={i} className="mono muted">{t.TaskPath}{t.TaskName} ({t.State})</div>
          ))}
        </div>
      </Card>

      {mode === "expert" && (
        <Card title="Raw security data (expert)" style={{ marginTop: 14 }}>
          <RawJson data={sec} />
        </Card>
      )}
    </>
  );
}
