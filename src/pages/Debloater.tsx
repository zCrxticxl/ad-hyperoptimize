import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Spinner } from "../components/ui";
import { HwWarnings } from "../components/HwWarnings";

const BLOAT_PACKAGES: { name: string; display: string; cat: string }[] = [
  { name: "Microsoft.BingNews",                     display: "Microsoft News",          cat: "Microsoft" },
  { name: "Microsoft.BingWeather",                  display: "Microsoft Weather",        cat: "Microsoft" },
  { name: "Microsoft.BingFinance",                  display: "Microsoft Finance",        cat: "Microsoft" },
  { name: "Microsoft.BingSports",                   display: "Microsoft Sports",         cat: "Microsoft" },
  { name: "Microsoft.MicrosoftSolitaireCollection", display: "Solitaire Collection",     cat: "Microsoft" },
  { name: "Microsoft.MixedReality.Portal",          display: "Mixed Reality Portal",     cat: "Microsoft" },
  { name: "Microsoft.People",                       display: "Microsoft People",          cat: "Microsoft" },
  { name: "Microsoft.SkypeApp",                     display: "Skype",                    cat: "Microsoft" },
  { name: "Microsoft.Teams",                        display: "Teams (Personal)",          cat: "Microsoft" },
  { name: "Microsoft.ZuneMusic",                    display: "Groove Music",             cat: "Microsoft" },
  { name: "Microsoft.ZuneVideo",                    display: "Movies & TV",              cat: "Microsoft" },
  { name: "Microsoft.GetHelp",                      display: "Get Help",                 cat: "Microsoft" },
  { name: "Microsoft.Getstarted",                   display: "Tips / Get Started",       cat: "Microsoft" },
  { name: "Microsoft.Microsoft3DViewer",            display: "3D Viewer",                cat: "Microsoft" },
  { name: "Microsoft.XboxApp",                      display: "Xbox (old)",               cat: "Xbox"      },
  { name: "Microsoft.XboxGameOverlay",              display: "Xbox Game Overlay",        cat: "Xbox"      },
  { name: "Microsoft.XboxGamingOverlay",            display: "Xbox Gaming Overlay",      cat: "Xbox"      },
  { name: "Microsoft.XboxIdentityProvider",         display: "Xbox Identity Provider",   cat: "Xbox"      },
  { name: "king.com.CandyCrushSaga",                display: "Candy Crush Saga",         cat: "3rd Party" },
  { name: "king.com.CandyCrushFriends",             display: "Candy Crush Friends",      cat: "3rd Party" },
  { name: "BytedancePte.Ltd.TikTok",                display: "TikTok",                   cat: "3rd Party" },
  { name: "SpotifyAB.SpotifyMusic",                 display: "Spotify (Store)",          cat: "3rd Party" },
  { name: "Disney.37853D22215E2",                   display: "Disney+",                  cat: "3rd Party" },
  { name: "AmazonVideo.PrimeVideo",                 display: "Prime Video",              cat: "3rd Party" },
];

const RECOMMENDED_BLOAT = [
  "Microsoft.BingNews","Microsoft.BingWeather","Microsoft.BingFinance",
  "Microsoft.BingSports","Microsoft.MicrosoftSolitaireCollection",
  "Microsoft.MixedReality.Portal","Microsoft.People","Microsoft.SkypeApp",
  "Microsoft.Microsoft3DViewer","Microsoft.Getstarted","Microsoft.GetHelp",
  "king.com.CandyCrushSaga","king.com.CandyCrushFriends","BytedancePte.Ltd.TikTok",
];

type Tweak = { id: string; name: string; desc: string; cat: string; applied: boolean };

function ProgressBar({ done, total }: { done: number; total: number }) {
  const pct = total ? Math.round((done / total) * 100) : 0;
  const color = pct === 100 ? "var(--green)" : pct > 50 ? "var(--yellow)" : "var(--accent)";
  return (
    <div style={{ background: "var(--bg2)", borderRadius: 6, height: 8, overflow: "hidden", marginBottom: 4 }}>
      <div style={{ width: `${pct}%`, height: "100%", background: color, borderRadius: 6, transition: "width 0.4s" }} />
    </div>
  );
}

export default function Debloater({ admin }: { admin: boolean }) {
  const [tweaks, setTweaks]       = useState<Tweak[]>([]);
  const [loading, setLoading]     = useState(true);
  const [busy, setBusy]           = useState<Record<string, boolean>>({});
  const [log, setLog]             = useState<{ msg: string; ok: boolean } | null>(null);
  const [applyingAll, setApplyingAll] = useState(false);
  const [selBloat, setSelBloat]   = useState<Set<string>>(new Set(RECOMMENDED_BLOAT));
  const [removingBloat, setRemovingBloat] = useState(false);
  const [bloatLog, setBloatLog]   = useState("");

  useEffect(() => {
    api.debloaterTweaksList().then(d => { setTweaks(d?.tweaks ?? []); setLoading(false); });
  }, []);

  const applied   = tweaks.filter(t => t.applied).length;
  const unapplied = tweaks.filter(t => !t.applied);

  const requireAdmin = () => {
    if (!admin) { setLog({ msg: "Administrator rights required.", ok: false }); return false; }
    return true;
  };

  const toggle = async (t: Tweak) => {
    if (!requireAdmin()) return;
    setBusy(p => ({ ...p, [t.id]: true }));
    try {
      const msg = t.applied ? await api.debloaterTweakRevert(t.id) : await api.debloaterTweakApply(t.id);
      setTweaks(prev => prev.map(tw => tw.id === t.id ? { ...tw, applied: !t.applied } : tw));
      setLog({ msg, ok: true });
    } catch (e: any) { setLog({ msg: String(e), ok: false }); }
    finally { setBusy(p => ({ ...p, [t.id]: false })); }
  };

  const applySection = async (ids: string[]) => {
    if (!requireAdmin()) return;
    setApplyingAll(true);
    for (const id of ids) {
      const t = tweaks.find(tw => tw.id === id && !tw.applied);
      if (!t) continue;
      try { await api.debloaterTweakApply(t.id); setTweaks(prev => prev.map(tw => tw.id === id ? { ...tw, applied: true } : tw)); } catch {}
    }
    setApplyingAll(false);
    setLog({ msg: "Section applied.", ok: true });
  };

  const applyAll = async () => {
    if (!requireAdmin()) return;
    setApplyingAll(true); setLog(null);
    let n = 0;
    for (const t of unapplied) {
      try { await api.debloaterTweakApply(t.id); setTweaks(prev => prev.map(tw => tw.id === t.id ? { ...tw, applied: true } : tw)); n++; } catch {}
    }
    setApplyingAll(false);
    setLog({ msg: `Applied ${n} tweak${n !== 1 ? "s" : ""}. Restart recommended.`, ok: true });
  };

  const removeBloat = async () => {
    if (!requireAdmin()) return;
    setRemovingBloat(true); setBloatLog("");
    let removed = 0, failed = 0;
    for (const name of selBloat) {
      try { await api.debloaterRemoveProvisioned(name); removed++; } catch { failed++; }
    }
    setRemovingBloat(false);
    setBloatLog(`Removed ${removed} app${removed !== 1 ? "s" : ""}${failed ? `, ${failed} skipped (already removed or not found)` : ""}.`);
  };

  const CAT_ORDER = ["Telemetry", "Ads & Clutter", "Privacy", "Gaming"];
  const byCategory = tweaks.reduce<Record<string, Tweak[]>>((acc, t) => { (acc[t.cat] ??= []).push(t); return acc; }, {});
  const sortedCategories = CAT_ORDER.filter(c => byCategory[c]).concat(Object.keys(byCategory).filter(c => !CAT_ORDER.includes(c)));
  const CAT_ICON: Record<string, string> = { Telemetry: "📡", "Ads & Clutter": "📢", Privacy: "🔒", Gaming: "🎮" };
  const CAT_RECOMMENDED: Record<string, boolean> = { Telemetry: true, "Ads & Clutter": true, Privacy: true, Gaming: false };

  return (
    <>
      <div className="page-title">🚀 Quick Setup</div>
      <div className="page-sub">
        Fresh install checklist — privacy, telemetry and bloatware in one place.
        Everything is reversible. Restart after applying for best results.
        {!admin && <span style={{ color: "var(--orange)" }}> · Admin required.</span>}
      </div>

      <HwWarnings page="debloater" />

      {/* Hero */}
      <div style={{ background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 10, padding: "18px 20px", marginBottom: 18 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 16, marginBottom: 10 }}>
          <div style={{ flex: 1 }}>
            <div style={{ fontWeight: 700, fontSize: 15, marginBottom: 6 }}>
              {loading ? "Scanning system…" : applied === tweaks.length && tweaks.length > 0
                ? "✅ All tweaks applied — system is clean"
                : `${applied} / ${tweaks.length} tweaks applied`}
            </div>
            {!loading && <ProgressBar done={applied} total={tweaks.length} />}
            <div className="muted" style={{ fontSize: 11 }}>
              {unapplied.length > 0 ? `${unapplied.length} remaining` : tweaks.length > 0 ? "Nothing left to do" : ""}
            </div>
          </div>
          {!loading && unapplied.length > 0 && (
            <button className="btn" style={{ minWidth: 200, fontSize: 14, padding: "10px 18px" }} onClick={applyAll} disabled={applyingAll || !admin}>
              {applyingAll ? <><Spinner /> Applying…</> : `⚡ Apply All (${unapplied.length})`}
            </button>
          )}
        </div>
        {log && (
          <div style={{ fontSize: 12, padding: "6px 10px", borderRadius: 6, background: log.ok ? "rgba(0,200,80,0.08)" : "rgba(255,60,60,0.08)", color: log.ok ? "var(--green)" : "var(--red)", border: `1px solid ${log.ok ? "var(--green)" : "var(--red)"}` }}>
            {log.msg}
          </div>
        )}
      </div>

      {loading && <div style={{ display: "flex", gap: 10, alignItems: "center" }}><Spinner /><span className="muted">Scanning…</span></div>}

      {/* Tweak sections */}
      {!loading && sortedCategories.map(cat => {
        const items = byCategory[cat];
        if (!items) return null;
        const catUnapplied = items.filter(t => !t.applied).map(t => t.id);
        const allDone = catUnapplied.length === 0;
        const isRecommended = CAT_RECOMMENDED[cat] ?? false;
        return (
          <div key={cat} style={{ marginBottom: 20 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 8 }}>
              <span style={{ fontSize: 11, fontWeight: 700, textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--muted)" }}>
                {CAT_ICON[cat] ?? "⚙"} {cat}
              </span>
              {isRecommended && <span style={{ fontSize: 10, color: "var(--green)", background: "rgba(0,200,80,0.1)", padding: "1px 6px", borderRadius: 8, fontWeight: 700 }}>Recommended</span>}
              <div style={{ flex: 1, borderBottom: "1px solid var(--border)" }} />
              {allDone
                ? <span style={{ fontSize: 11, color: "var(--green)" }}>✓ all applied</span>
                : <button className="btn small ghost" style={{ fontSize: 11 }} onClick={() => applySection(catUnapplied)} disabled={applyingAll}>Apply section →</button>}
            </div>
            {items.map(t => (
              <div key={t.id} style={{ display: "flex", alignItems: "center", gap: 12, padding: "8px 0", borderBottom: "1px solid var(--border)", opacity: busy[t.id] ? 0.7 : 1 }}>
                <button
                  onClick={() => toggle(t)}
                  style={{ width: 22, height: 22, borderRadius: 5, flexShrink: 0, border: `2px solid ${t.applied ? "var(--green)" : "var(--border)"}`, background: t.applied ? "var(--green)" : "transparent", color: "#fff", fontSize: 12, display: "flex", alignItems: "center", justifyContent: "center", cursor: admin ? "pointer" : "not-allowed" }}
                >
                  {busy[t.id] ? <Spinner /> : t.applied ? "✓" : ""}
                </button>
                <div style={{ flex: 1 }}>
                  <div style={{ fontWeight: 600, fontSize: 13 }}>{t.name}</div>
                  <div className="muted" style={{ fontSize: 11 }}>{t.desc}</div>
                </div>
                <span style={{ fontSize: 11, fontWeight: 600, color: t.applied ? "var(--green)" : "var(--muted)" }}>
                  {t.applied ? "Applied" : "Not applied"}
                </span>
              </div>
            ))}
          </div>
        );
      })}

      {/* UWP Bloatware */}
      <div style={{ marginTop: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 10 }}>
          <span style={{ fontSize: 11, fontWeight: 700, textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--muted)" }}>🗑 Remove UWP Bloatware</span>
          <span style={{ fontSize: 10, color: "var(--accent)", background: "rgba(0,140,255,0.1)", padding: "1px 6px", borderRadius: 8 }}>Optional</span>
          <div style={{ flex: 1, borderBottom: "1px solid var(--border)" }} />
        </div>
        <div className="muted" style={{ fontSize: 12, marginBottom: 12 }}>Pre-selected = most commonly removed. Uncheck anything you want to keep.</div>

        {["3rd Party", "Microsoft", "Xbox"].map(grp => {
          const items = BLOAT_PACKAGES.filter(b => b.cat === grp);
          if (!items.length) return null;
          return (
            <div key={grp} style={{ marginBottom: 12 }}>
              <div style={{ fontSize: 11, fontWeight: 700, color: "var(--muted)", marginBottom: 6, textTransform: "uppercase", letterSpacing: "0.07em" }}>{grp}</div>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                {items.map(b => {
                  const checked = selBloat.has(b.name);
                  return (
                    <label key={b.name} style={{ display: "flex", alignItems: "center", gap: 6, padding: "4px 10px", borderRadius: 6, cursor: "pointer", border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`, background: checked ? "rgba(0,140,255,0.07)" : "var(--bg2)", fontSize: 12, fontWeight: 500, userSelect: "none" }}>
                      <input type="checkbox" checked={checked} onChange={() => setSelBloat(prev => { const n = new Set(prev); n.has(b.name) ? n.delete(b.name) : n.add(b.name); return n; })} style={{ pointerEvents: "none" }} />
                      {b.display}
                    </label>
                  );
                })}
              </div>
            </div>
          );
        })}

        <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 12 }}>
          <button className="btn danger" disabled={!selBloat.size || removingBloat || !admin} onClick={removeBloat}>
            {removingBloat ? <><Spinner /> Removing…</> : `🗑 Remove (${selBloat.size})`}
          </button>
          <button className="btn small ghost" onClick={() => setSelBloat(new Set(RECOMMENDED_BLOAT))}>Reset</button>
          <button className="btn small ghost" onClick={() => setSelBloat(new Set(BLOAT_PACKAGES.map(b => b.name)))}>All</button>
          <button className="btn small ghost" onClick={() => setSelBloat(new Set())}>None</button>
        </div>
        {bloatLog && <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>{bloatLog}</div>}
      </div>

      <div className="muted" style={{ fontSize: 11, marginTop: 20 }}>
        System tweaks are reversible — click an applied tweak to revert. UWP removal is permanent but apps can be reinstalled from the Microsoft Store.
      </div>
    </>
  );
}
