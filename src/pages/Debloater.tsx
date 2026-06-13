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
    const fn = t.applied
      ? () => api.debloaterTweakRevert(t.id)
      : () => api.debloaterTweakApply(t.id);
    await act(t.id, fn);
    api.debloaterTweaksList().then(setTweaks);
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
      <div className="row" style={{ gap: 8, marginBottom: 16 }}>
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
                        <span style={{
                          fontSize: 10, padding: "1px 6px", borderRadius: 4,
                          background: CAT_COLORS[t.cat] ?? "var(--bg3)",
                          color: "#fff", opacity: 0.85,
                        }}>{t.cat}</span>
                      </div>
                      <div className="muted" style={{ fontSize: 12 }}>{t.desc}</div>
                    </div>
                    <button
                      className={`btn small ${t.applied ? "ghost danger" : ""}`}
                      style={{ flexShrink: 0 }}
                      disabled={busy === t.id}
                      onClick={() => tweak_toggle(t)}
                    >
                      {busy === t.id ? <><Spinner /> …</> : t.applied ? "Revert" : "Apply"}
                    </button>
                  </div>
                ))}
              </Card>
            ))}
          </>
        )
      )}

      {tab === "uwp" && (
        <Card title={`UWP Apps (${uwp_list.length})`}>
          {!uwp ? <Spinner /> : (
            <>
              <input
                placeholder={`Search ${uwp_list.length} apps…`}
                value={uwpSearch}
                onChange={e => setUwpSearch(e.target.value)}
                style={{ width: "100%", padding: "5px 10px", fontSize: 13, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)", marginBottom: 12 }}
              />
              <div style={{ maxHeight: "60vh", overflowY: "auto" }}>
                {uwp_filtered.map((app: any) => (
                  <div key={app.PackageFullName} className="row" style={{ padding: "6px 0", borderBottom: "1px solid var(--border)", gap: 10, alignItems: "center" }}>
                    <div style={{ flex: 1 }}>
                      <div style={{ fontSize: 13, fontWeight: 600 }}>{app.Name}</div>
                      <div className="muted" style={{ fontSize: 11 }}>{app.Publisher}</div>
                    </div>
                    <button
                      className="btn small ghost danger"
                      disabled={busy === app.PackageFullName}
                      onClick={() => act(app.PackageFullName, async () => {
                        const r = await api.debloaterRemoveUwp(app.PackageFullName);
                        api.debloaterUwpList().then(setUwp);
                        return r;
                      })}
                    >
                      {busy === app.PackageFullName ? "…" : "Remove"}
                    </button>
                  </div>
                ))}
              </div>
            </>
          )}
        </Card>
      )}

      {log && (
        <div style={{
          marginTop: 12, padding: "10px 14px", borderRadius: 6, fontSize: 13,
          background: log.ok ? "rgba(80,200,120,0.08)" : "rgba(255,80,80,0.08)",
          border: `1px solid ${log.ok ? "var(--green)" : "var(--red)"}`,
          color: log.ok ? "var(--green)" : "var(--red)",
        }}>
          {log.ok ? "✓ " : "✗ "}{log.msg}
        </div>
      )}

      <div className="muted" style={{ fontSize: 12, marginTop: 14 }}>
        Tweaks are stored in registry and can be reverted at any time. UWP removal is permanent (can be
        reinstalled from the Microsoft Store).
      </div>
    </>
  );
}
