import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Spinner } from "../components/ui";
import { HwWarnings } from "../components/HwWarnings";
import { useLang } from "../i18n";

const BLOAT_PACKAGES: { name: string; displayKey: string; cat: string }[] = [
  { name: "Microsoft.BingNews",                     displayKey: "debloatPkgBingNews",        cat: "Microsoft" },
  { name: "Microsoft.BingWeather",                  displayKey: "debloatPkgBingWeather",     cat: "Microsoft" },
  { name: "Microsoft.BingFinance",                  displayKey: "debloatPkgBingFinance",     cat: "Microsoft" },
  { name: "Microsoft.BingSports",                   displayKey: "debloatPkgBingSports",      cat: "Microsoft" },
  { name: "Microsoft.MicrosoftSolitaireCollection", displayKey: "debloatPkgSolitaire",       cat: "Microsoft" },
  { name: "Microsoft.MixedReality.Portal",          displayKey: "debloatPkgMixedReality",    cat: "Microsoft" },
  { name: "Microsoft.People",                       displayKey: "debloatPkgPeople",          cat: "Microsoft" },
  { name: "Microsoft.SkypeApp",                     displayKey: "debloatPkgSkype",           cat: "Microsoft" },
  { name: "Microsoft.Teams",                        displayKey: "debloatPkgTeams",           cat: "Microsoft" },
  { name: "Microsoft.ZuneMusic",                    displayKey: "debloatPkgGrooveMusic",     cat: "Microsoft" },
  { name: "Microsoft.ZuneVideo",                    displayKey: "debloatPkgMoviesTv",        cat: "Microsoft" },
  { name: "Microsoft.GetHelp",                      displayKey: "debloatPkgGetHelp",         cat: "Microsoft" },
  { name: "Microsoft.Getstarted",                   displayKey: "debloatPkgGetStarted",      cat: "Microsoft" },
  { name: "Microsoft.Microsoft3DViewer",            displayKey: "debloatPkg3dViewer",        cat: "Microsoft" },
  { name: "Microsoft.XboxApp",                      displayKey: "debloatPkgXboxOld",         cat: "Xbox"      },
  { name: "Microsoft.XboxGameOverlay",              displayKey: "debloatPkgXboxGameOverlay", cat: "Xbox"      },
  { name: "Microsoft.XboxGamingOverlay",            displayKey: "debloatPkgXboxGamingOverlay", cat: "Xbox"      },
  { name: "Microsoft.XboxIdentityProvider",         displayKey: "debloatPkgXboxIdentity",    cat: "Xbox"      },
  { name: "king.com.CandyCrushSaga",                displayKey: "debloatPkgCandyCrushSaga",  cat: "3rd Party" },
  { name: "king.com.CandyCrushFriends",             displayKey: "debloatPkgCandyCrushFriends", cat: "3rd Party" },
  { name: "BytedancePte.Ltd.TikTok",                displayKey: "debloatPkgTikTok",          cat: "3rd Party" },
  { name: "SpotifyAB.SpotifyMusic",                 displayKey: "debloatPkgSpotify",         cat: "3rd Party" },
  { name: "Disney.37853D22215E2",                   displayKey: "debloatPkgDisneyPlus",      cat: "3rd Party" },
  { name: "AmazonVideo.PrimeVideo",                 displayKey: "debloatPkgPrimeVideo",      cat: "3rd Party" },
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
  const { t } = useLang();
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
    if (!admin) { setLog({ msg: t("debloatAdminRequiredMsg"), ok: false }); return false; }
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
    setLog({ msg: t("debloatSectionApplied"), ok: true });
  };

  const applyAll = async () => {
    if (!requireAdmin()) return;
    setApplyingAll(true); setLog(null);
    let n = 0;
    for (const t of unapplied) {
      try { await api.debloaterTweakApply(t.id); setTweaks(prev => prev.map(tw => tw.id === t.id ? { ...tw, applied: true } : tw)); n++; } catch {}
    }
    setApplyingAll(false);
    setLog({ msg: `${t("debloatAppliedN")} ${n} ${n !== 1 ? t("debloatTweaksWord") : t("debloatTweakWord")}. ${t("debloatRestartRecommended")}`, ok: true });
  };

  const removeBloat = async () => {
    if (!requireAdmin()) return;
    setRemovingBloat(true); setBloatLog("");
    let removed = 0, failed = 0;
    for (const name of selBloat) {
      try { await api.debloaterRemoveProvisioned(name); removed++; } catch { failed++; }
    }
    setRemovingBloat(false);
    setBloatLog(`${t("debloatRemovedN")} ${removed} ${removed !== 1 ? t("debloatAppsWord") : t("debloatAppWord")}${failed ? `, ${failed} ${t("debloatSkipped")}` : ""}.`);
  };

  const CAT_ORDER = ["Telemetry", "Ads & Clutter", "Privacy", "Gaming"];
  const byCategory = tweaks.reduce<Record<string, Tweak[]>>((acc, t) => { (acc[t.cat] ??= []).push(t); return acc; }, {});
  const sortedCategories = CAT_ORDER.filter(c => byCategory[c]).concat(Object.keys(byCategory).filter(c => !CAT_ORDER.includes(c)));
  const CAT_ICON: Record<string, string> = { Telemetry: "📡", "Ads & Clutter": "📢", Privacy: "🔒", Gaming: "🎮" };
  const CAT_RECOMMENDED: Record<string, boolean> = { Telemetry: true, "Ads & Clutter": true, Privacy: true, Gaming: false };
  const CAT_LABEL: Record<string, string> = { Telemetry: t("debloatCatTelemetry"), "Ads & Clutter": t("debloatCatAdsClutter"), Privacy: t("debloatCatPrivacy"), Gaming: t("debloatCatGaming") };
  const GRP_LABEL: Record<string, string> = { "3rd Party": t("debloatGrp3rdParty"), Microsoft: t("debloatGrpMicrosoft"), Xbox: t("debloatGrpXbox") };

  return (
    <>
      <div className="page-title">🚀 {t("debloatTitle")}</div>
      <div className="page-sub">
        {t("debloatSub")}
        {!admin && <span style={{ color: "var(--orange)" }}> · {t("debloatAdminRequired")}</span>}
      </div>

      <HwWarnings page="debloater" />

      {/* Hero */}
      <div style={{ background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 10, padding: "18px 20px", marginBottom: 18 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 16, marginBottom: 10 }}>
          <div style={{ flex: 1 }}>
            <div style={{ fontWeight: 700, fontSize: 15, marginBottom: 6 }}>
              {loading ? t("debloatScanningSystem") : applied === tweaks.length && tweaks.length > 0
                ? `✅ ${t("debloatAllApplied")}`
                : `${applied} / ${tweaks.length} ${t("debloatTweaksApplied")}`}
            </div>
            {!loading && <ProgressBar done={applied} total={tweaks.length} />}
            <div className="muted" style={{ fontSize: 11 }}>
              {unapplied.length > 0 ? `${unapplied.length} ${t("debloatRemaining")}` : tweaks.length > 0 ? t("debloatNothingLeft") : ""}
            </div>
          </div>
          {!loading && unapplied.length > 0 && (
            <button className="btn" style={{ minWidth: 200, fontSize: 14, padding: "10px 18px" }} onClick={applyAll} disabled={applyingAll || !admin}>
              {applyingAll ? <><Spinner /> {t("debloatApplying")}</> : `⚡ ${t("debloatApplyAllBtn")} (${unapplied.length})`}
            </button>
          )}
        </div>
        {log && (
          <div style={{ fontSize: 12, padding: "6px 10px", borderRadius: 6, background: log.ok ? "rgba(0,200,80,0.08)" : "rgba(255,60,60,0.08)", color: log.ok ? "var(--green)" : "var(--red)", border: `1px solid ${log.ok ? "var(--green)" : "var(--red)"}` }}>
            {log.msg}
          </div>
        )}
      </div>

      {loading && <div style={{ display: "flex", gap: 10, alignItems: "center" }}><Spinner /><span className="muted">{t("debloatScanningEllipsis")}</span></div>}

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
                {CAT_ICON[cat] ?? "⚙"} {CAT_LABEL[cat] ?? cat}
              </span>
              {isRecommended && <span style={{ fontSize: 10, color: "var(--green)", background: "rgba(0,200,80,0.1)", padding: "1px 6px", borderRadius: 8, fontWeight: 700 }}>{t("debloatRecommended")}</span>}
              <div style={{ flex: 1, borderBottom: "1px solid var(--border)" }} />
              {allDone
                ? <span style={{ fontSize: 11, color: "var(--green)" }}>✓ {t("debloatAllAppliedShort")}</span>
                : <button className="btn small ghost" style={{ fontSize: 11 }} onClick={() => applySection(catUnapplied)} disabled={applyingAll}>{t("debloatApplySection")} →</button>}
            </div>
            {items.map(tw => (
              <div key={tw.id} style={{ display: "flex", alignItems: "center", gap: 12, padding: "8px 0", borderBottom: "1px solid var(--border)", opacity: busy[tw.id] ? 0.7 : 1 }}>
                <button
                  onClick={() => toggle(tw)}
                  style={{ width: 22, height: 22, borderRadius: 5, flexShrink: 0, border: `2px solid ${tw.applied ? "var(--green)" : "var(--border)"}`, background: tw.applied ? "var(--green)" : "transparent", color: "#fff", fontSize: 12, display: "flex", alignItems: "center", justifyContent: "center", cursor: admin ? "pointer" : "not-allowed" }}
                >
                  {busy[tw.id] ? <Spinner /> : tw.applied ? "✓" : ""}
                </button>
                <div style={{ flex: 1 }}>
                  <div style={{ fontWeight: 600, fontSize: 13 }}>{tw.name}</div>
                  <div className="muted" style={{ fontSize: 11 }}>{tw.desc}</div>
                </div>
                <span style={{ fontSize: 11, fontWeight: 600, color: tw.applied ? "var(--green)" : "var(--muted)" }}>
                  {tw.applied ? t("debloatApplied") : t("debloatNotApplied")}
                </span>
              </div>
            ))}
          </div>
        );
      })}

      {/* UWP Bloatware */}
      <div style={{ marginTop: 8 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 10 }}>
          <span style={{ fontSize: 11, fontWeight: 700, textTransform: "uppercase", letterSpacing: "0.07em", color: "var(--muted)" }}>🗑 {t("debloatRemoveUwp")}</span>
          <span style={{ fontSize: 10, color: "var(--accent)", background: "rgba(0,140,255,0.1)", padding: "1px 6px", borderRadius: 8 }}>{t("debloatOptional")}</span>
          <div style={{ flex: 1, borderBottom: "1px solid var(--border)" }} />
        </div>
        <div className="muted" style={{ fontSize: 12, marginBottom: 12 }}>{t("debloatPreselectedNote")}</div>

        {["3rd Party", "Microsoft", "Xbox"].map(grp => {
          const items = BLOAT_PACKAGES.filter(b => b.cat === grp);
          if (!items.length) return null;
          return (
            <div key={grp} style={{ marginBottom: 12 }}>
              <div style={{ fontSize: 11, fontWeight: 700, color: "var(--muted)", marginBottom: 6, textTransform: "uppercase", letterSpacing: "0.07em" }}>{GRP_LABEL[grp] ?? grp}</div>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                {items.map(b => {
                  const checked = selBloat.has(b.name);
                  return (
                    <label key={b.name} style={{ display: "flex", alignItems: "center", gap: 6, padding: "4px 10px", borderRadius: 6, cursor: "pointer", border: `1px solid ${checked ? "var(--accent)" : "var(--border)"}`, background: checked ? "rgba(0,140,255,0.07)" : "var(--bg2)", fontSize: 12, fontWeight: 500, userSelect: "none" }}>
                      <input type="checkbox" checked={checked} onChange={() => setSelBloat(prev => { const n = new Set(prev); n.has(b.name) ? n.delete(b.name) : n.add(b.name); return n; })} style={{ pointerEvents: "none" }} />
                      {t(b.displayKey as any)}
                    </label>
                  );
                })}
              </div>
            </div>
          );
        })}

        <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 12 }}>
          <button className="btn danger" disabled={!selBloat.size || removingBloat || !admin} onClick={removeBloat}>
            {removingBloat ? <><Spinner /> {t("debloatRemoving")}</> : `🗑 ${t("debloatRemoveBtn")} (${selBloat.size})`}
          </button>
          <button className="btn small ghost" onClick={() => setSelBloat(new Set(RECOMMENDED_BLOAT))}>{t("debloatReset")}</button>
          <button className="btn small ghost" onClick={() => setSelBloat(new Set(BLOAT_PACKAGES.map(b => b.name)))}>{t("debloatAll")}</button>
          <button className="btn small ghost" onClick={() => setSelBloat(new Set())}>{t("debloatNone")}</button>
        </div>
        {bloatLog && <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>{bloatLog}</div>}
      </div>

      <div className="muted" style={{ fontSize: 11, marginTop: 20 }}>
        {t("debloatFooterNote")}
      </div>
    </>
  );
}
