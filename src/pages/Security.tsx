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
    <span style={{ color: on ? "var(--green)" : "var(--red)", fontWeight: 700 }}>
      {on ? "✓ On" : "✗ Off"}
    </span>
  );

  const fwArr: any[] = Array.isArray(sec.firewall) ? sec.firewall : [];
  const fwMap: Record<string, any> = Object.fromEntries(fwArr.map((f: any) => [f.Name, f]));

  const rows = [
    { label: "Real-Time Protection",    val: light(d.RealTimeProtectionEnabled) },
    { label: "Cloud Protection",        val: light(d.MAPSReporting) },
    { label: "Tamper Protection",       val: light(d.TamperProtectionSource) },
    { label: "Firewall (Domain)",  val: light(fwMap["Domain"]?.Enabled) },
    { label: "Firewall (Private)", val: light(fwMap["Private"]?.Enabled) },
    { label: "Firewall (Public)",  val: light(fwMap["Public"]?.Enabled) },
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

      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
        {/* Defender + Firewall */}
        <Card title="Windows Defender & Firewall">
          {rows.map(r => (
            <div key={r.label} className="row" style={{ justifyContent: "space-between", padding: "5px 0", borderBottom: "1px solid var(--border)", fontSize: 13 }}>
              <span className="muted">{r.label}</span>
              {r.val}
            </div>
          ))}
        </Card>

        {/* Drivers */}
        <Card title={`Unsigned / Suspicious Drivers (${susDrivers.length})`}>
          {susDrivers.length === 0 ? (
            <span className="muted" style={{ fontSize: 13 }}>✓ No suspicious drivers found.</span>
          ) : susDrivers.map((dr: any) => (
            <div key={dr.name} style={{ fontSize: 12, padding: "4px 0", borderBottom: "1px solid var(--border)" }}>
              <div style={{ fontWeight: 600 }}>{dr.name}</div>
              <div className="muted">{dr.path}</div>
              {dr.signer && <div style={{ color: "var(--orange)" }}>Signer: {dr.signer}</div>}
            </div>
          ))}
        </Card>
      </div>

      <div style={{ marginTop: 12, display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12 }}>
        {/* Autoruns */}
        <Card title={`Suspicious Autoruns (${susAutoruns.length})`}>
          {susAutoruns.length === 0 ? (
            <span className="muted" style={{ fontSize: 13 }}>✓ No suspicious startup entries found.</span>
          ) : susAutoruns.map((a: any, i: number) => (
            <div key={i} style={{ fontSize: 12, padding: "4px 0", borderBottom: "1px solid var(--border)" }}>
              <div style={{ fontWeight: 600 }}>{a.Name ?? a.name ?? a.TaskName}</div>
              <div className="muted" style={{ wordBreak: "break-all" }}>{a.FullName ?? a.path ?? a.TaskPath}</div>
            </div>
          ))}
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
