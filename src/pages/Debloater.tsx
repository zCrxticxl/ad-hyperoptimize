import React, { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";

const CAT_COLORS: Record<string, string> = {
  Telemetry: "var(--red)",
  Privacy:   "var(--orange)",
  Gaming:    "var(--green)",
};

export default function Debloater({ admin }: { admin: boolean }) {
  const [tab, setTab]           = useState<"tweaks" | "uwp">("tweaks");
  const [tweaks, setTweaks]     = useState<any>(null);
  const [uwp, setUwp]           = useState<any>(null);
  const [uwpSearch, setUwpSearch] = useState("");
  const [busy, setBusy]         = useState<string | null>(null);
  const [log, setLog]           = useState<{ msg: string; ok: boolean } | null>(null);

  useEffect(() => {
    if (tab === "tweaks" && !tweaks) api.debloaterTweaksList().then(setTweaks);
    if (tab === "uwp" && !uwp) api.debloaterUwpList().then(setUwp);
  }, [tab]);

  const act = async (label: string, fn: () => Promise<string>) => {
    if (!admin) { setLog({ msg: "Administrator rights required. Restart the app as admin.", ok: false }); return; }
    setBusy(label);
    setLog(null);
    try { setLog({ msg: await fn(), ok: true }); }
    catch (e: any) { setLog({ msg: String(e), ok: false }); }
    finally { setBusy(null); }
  };

  const tweak_toggle = async (t: any) => {
    if (!admin) { setLog({ msg: "Administrator rights required. Restart the app as admin.", ok: false }); return; }
    setBusy(t.id);
    setLog(null);
    try {
      const msg = t.applied
        ? await api.debloaterTweakRevert(t.id)
        : await api.debloaterTweakApply(t.id);
      setLog({ msg, ok: true });
      // Flip state immediately so user sees the change right away
      setTweaks((prev: any) => prev ? ({
        ...prev,
        tweaks: prev.tweaks.map((tw: any) =>
          tw.id === t.id ? { ...tw, applied: !t.applied } : tw
        )
      }) : prev);
    } catch (e: any) {
      setLog({ msg: String(e), ok: false });
      // Re-check on error to show accurate state
      api.debloaterTweaksList().then(setTweaks);
    } finally {
      setBusy(null);
    }
  };

  const tweak_list: any[] = tweaks?.tweaks ?? [];
  const uwp_list:   any[] = uwp?.apps ?? [];

  const uwp_filtered = useMemo(() => {
    if (!uwpSearch.trim()) return uwp_list;
    const q = uwpSearch.toLowerCase();
    return uwp_list.filter((a: any) =>
      a.Name?.toLowerCase().includes(q) || a.Publisher?.toLowerCase().includes(q)
    );
  }, [uwp_list, uwpSearch]);

  // Group tweaks by category
  const grouped = tweak_list.reduce((acc: any, t: any) => {
    (acc[t.cat] ??= []).push(t);
    return acc;
  }, {} as Record<string, any[]>);

  return (
    <>
      <div className="page-title">🧼 Windows Debloater</div>
      <div className="page-sub">
        Disable telemetry, ads, Cortana, and remove pre-installed bloatware. All tweaks are reversible.
        {!admin && <span style={{ color: "var(--orange)" }}> · Admin required for system tweaks.</span>}
      </div>

      {/* Tab bar */}
      <div className="row" style={{ gap: 8, marginBottom: log ? 8 : 16 }}>
        {(["tweaks", "uwp"] as const).map(t => (
          <button
            key={t}
            className={`btn small ${tab === t ? "" : "ghost"}`}
            onClick={() => setTab(t)}
          >
            {t === "tweaks" ? "⚙ System Tweaks" : "📦 Remove UWP Apps"}
          </button>
        ))}
      </div>

      {/* Log banner — pinned near top so it's always visible */}
      {log && (
        <div style={{
          marginBottom: 14, padding: "10px 14px", borderRadius: 6, fontSize: 13,
          background: log.ok ? "rgba(80,200,120,0.10)" : "rgba(255,80,80,0.10)",
          border: `1px solid ${log.ok ? "var(--green)" : "var(--red)"}`,
          color: log.ok ? "var(--green)" : "var(--red)",
          display: "flex", alignItems: "center", gap: 10,
        }}>
          <span style={{ fontSize: 16 }}>{log.ok ? "✓" : "✗"}</span>
          <span style={{ flex: 1 }}>{log.msg}</span>
          <button onClick={() => setLog(null)} style={{ background: "none", border: "none", cursor: "pointer", color: "inherit", opacity: 0.6, fontSize: 16, lineHeight: 1 }}>×</button>
        </div>
      )}

      {tab === "tweaks" && (
        !tweaks ? (
          <Card title="">
            <div className="row" style={{ gap: 10 }}>
              <Spinner />
              <span className="muted">Checking current Windows settings…</span>
            </div>
          </Card>
        ) : (
          <>
            {/* Summary bar */}
            {(() => {
              const applied = tweak_list.filter(t => t.applied).length;
              const total   = tweak_list.length;
              return (
                <div style={{ marginBottom: 14, padding: "10px 14px", borderRadius: 6, background: "var(--bg2)", border: "1px solid var(--border)", display: "flex", gap: 16, alignItems: "center" }}>
                  <span style={{ fontSize: 13 }}>
                    <b style={{ color: "var(--green)" }}>{applied}</b>
                    <span className="muted"> / {total} tweaks already active on this system</span>
                  </span>
                  {applied === total && <span style={{ color: "var(--green)", fontSize: 12, fontWeight: 700 }}>✓ Fully optimized</span>}
                </div>
              );
            })()}

            {Object.entries(grouped).map(([cat, items]: [string, any]) => (
              <Card key={cat} title={cat} style={{ marginBottom: 14 }}>
                {items.map((t: any) => (
                  <div key={t.id} className="row" style={{ padding: "8px 0", borderBottom: "1px solid var(--border)", alignItems: "flex-start", gap: 12 }}>
                    <div style={{ flex: 1 }}>
                      <div className="row" style={{ gap: 8, alignItems: "center", marginBottom: 2 }}>
                        <span style={{
                          fontSize: 11, fontWeight: 700, minWidth: 90,
                          color: t.applied ? "var(--green)" : "var(--muted)",
                        }}>
                          {t.applied ? "✓ Active" : "✗ Not applied"}
                        </span>
                        <b style={{ fontSize: 13 }}>{t.name}</b>
                        <span style={{ color: CAT_COLORS[cat] ?? "var(--muted)", fontSize: 11, marginLeft: 6 }}>{t.cat}</span>
                      </div>
                      <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>{t.description}</div>
                    </div>
                    <button
                      className={`btn small ${t.applied ? "ghost" : ""}`}
                      disabled={busy === t.id}
                      onClick={() => tweak_toggle(t)}
                      style={{ minWidth: 72, flexShrink: 0 }}
                    >
                      {busy === t.id ? <Spinner /> : t.applied ? "Revert" : "Apply"}
                    </button>
                  </div>
                ))}
              </Card>
            ))}
          </>
        )
      )}

      {tab === "uwp" && (
        !uwp ? (
          <Card title="">
            <div className="row" style={{ gap: 10 }}>
              <Spinner />
              <span className="muted">Loading installed UWP apps…</span>
            </div>
          </Card>
        ) : (
          <>
            <div className="row" style={{ gap: 8, marginBottom: 12 }}>
              <input
                className="search-box"
                placeholder="Search apps…"
                value={uwpSearch}
                onChange={e => setUwpSearch(e.target.value)}
                style={{ flex: 1, padding: "7px 12px", borderRadius: 6, border: "1px solid var(--border)", background: "var(--bg2)", color: "var(--fg)", fontSize: 13 }}
              />
              <button
                className="btn small ghost"
                onClick={() => act("remove_all_bloat", () => api.debloaterRemoveUwp("ALL_BLOATWARE"))}
                disabled={!!busy}
              >
                {busy === "remove_all_bloat" ? <Spinner /> : "🗑 Remove All Bloatware"}
              </button>
            </div>

            <Card title={`📦 Installed UWP Apps (${uwp_filtered.length})`}>
              {uwp_filtered.length === 0 && (
                <div className="muted" style={{ fontSize: 13 }}>No apps found.</div>
              )}
              {uwp_filtered.map((a: any) => (
                <div key={a.PackageFullName ?? a.Name} className="row" style={{ padding: "7px 0", borderBottom: "1px solid var(--border)", alignItems: "center", gap: 10 }}>
                  <div style={{ flex: 1 }}>
                    <div style={{ fontSize: 13, fontWeight: 600 }}>{a.Name}</div>
                    <div className="muted" style={{ fontSize: 11 }}>{a.Publisher}</div>
                  </div>
                  <button
                    className="btn small ghost"
                    style={{ color: "var(--red)", borderColor: "var(--red)", flexShrink: 0 }}
                    disabled={busy === a.PackageFullName}
                    onClick={() => act(a.PackageFullName, () =>
                      api.debloaterRemoveUwp(a.PackageFullName)
                        .then((msg: string) => { setUwp((prev: any) => prev ? ({ ...prev, apps: prev.apps.filter((x: any) => x.PackageFullName !== a.PackageFullName) }) : prev); return msg; })
                    )}
                  >
                    {busy === a.PackageFullName ? <Spinner /> : "Remove"}
                  </button>
                </div>
              ))}
            </Card>
          </>
        )
      )}
    </>
  );
}
