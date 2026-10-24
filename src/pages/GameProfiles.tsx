import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { useLang } from "../i18n";
import { ErrorBanner } from "../components/ui";

export default function GameProfiles() {
  const { t } = useLang();
  const PRESET_LABELS: Record<string, { label: string; color: string; icon: string }> = {
    performance: { label: t("gameprofPresetPerformance"), color: "var(--yellow)", icon: "⚡" },
    balanced:    { label: t("gameprofPresetBalanced"),     color: "var(--accent)", icon: "⚖️" },
    quality:     { label: t("gameprofPresetQuality"),      color: "var(--accent2)", icon: "✨" },
  };

  const PLAN_LABELS: Record<string, string> = {
    ultimate:         t("gameprofPlanUltimate"),
    high_performance: t("gameprofPlanHighPerformance"),
    balanced:         t("gameprofPlanBalanced"),
    // gameprofile.rs BATTERY_NOTE
    "unchanged (battery device: game profiles do not switch the power plan)": t("gameprofPlanBattery"),
  };

  const [games, setGames]           = useState<any[]>([]);
  const [status, setStatus]         = useState<any>(null);
  const [selectedGame, setSelected] = useState<any>(null);
  const [preset, setPreset]         = useState<"performance"|"balanced"|"quality">("performance");
  const [filter, setFilter]         = useState("");
  const [busy, setBusy]             = useState(false);
  const [toast, setToast]           = useState<string | null>(null);
  const [activeEvt, setActiveEvt]   = useState<any>(null);
  const [loadErr, setLoadErr]       = useState("");
  const [lastResult, setLastResult] = useState<{ ok: boolean; text: string } | null>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3500);
  };

  const loadStatus = () =>
    api.gameSwitcherStatus().then(setStatus).catch((e: any) => setLoadErr(String(e)));
  const loadAll = () => {
    setLoadErr("");
    api.gameList().then((g) => setGames(Array.isArray(g) ? g : [])).catch((e: any) => setLoadErr(String(e)));
    loadStatus();
  };

  useEffect(() => {
    loadAll();

    const unSub1 = listen("game-detected", (e: any) => {
      setActiveEvt(e.payload);
      setStatus((s: any) => ({ ...s, activeGame: e.payload.id }));
      if (e.payload?.error) {
        showToast(`${t("gameprofToastError")}: ${e.payload.name ?? ""} ${e.payload.error}`);
      } else {
        showToast(`🎮 ${e.payload.name} ${t("gameprofToastDetected")}, ${e.payload.preset} ${t("gameprofToastProfileApplied")}${e.payload?.note ? ` (${e.payload.note})` : ""}`);
      }
    });
    const unSub2 = listen("game-exited", (e: any) => {
      setActiveEvt(null);
      setStatus((s: any) => ({ ...s, activeGame: null }));
      showToast(e.payload?.error
        ? `${t("gameprofToastError")}: ${e.payload.error}`
        : `🔁 ${t("gameprofToastGameClosed")}${e.payload?.note ? ` (${e.payload.note})` : ""}`);
    });
    return () => { unSub1.then(f => f()); unSub2.then(f => f()); };
  }, [t]);

  const filtered = useMemo(() =>
    games.filter(g =>
      !filter ||
      g.name.toLowerCase().includes(filter.toLowerCase()) ||
      g.genre.toLowerCase().includes(filter.toLowerCase())
    ), [games, filter]);

  const genres = useMemo(() =>
    [...new Set(games.map((g: any) => g.genre))],
    [games]);

  const toggleSwitcher = async () => {
    if (!status) return;
    const next = !status.enabled;
    try {
      await api.gameSwitcherConfigure(next, status.defaultPreset ?? "performance");
      setStatus((s: any) => ({ ...s, enabled: next }));
      showToast(next ? `✅ ${t("gameprofToastSwitcherEnabled")}` : `⏸ ${t("gameprofToastSwitcherDisabled")}`);
    } catch (e: any) { showToast(`${t("gameprofToastError")}: ${String(e)}`); }
  };

  const changeDefaultPreset = async (p: string) => {
    if (!status) return;
    try {
      await api.gameSwitcherConfigure(status.enabled, p);
      setStatus((s: any) => ({ ...s, defaultPreset: p }));
    } catch (e: any) { showToast(`${t("gameprofToastError")}: ${String(e)}`); }
  };

  const applyPreset = async (game: any, p: string) => {
    if (busy) return;
    setBusy(true);
    setLastResult(null);
    try {
      const res = await api.gameApplyPreset(game.id, p);
      // Show exactly what the backend reported, then re-read the live state.
      if (res?.ok) {
        const extra = [res.warning, res.note, res.error].filter(Boolean).join("; ");
        const text = `⚡ ${t("gameprofToastApplied")} ${p} ${t("gameprofToastPresetFor")} ${res.game ?? game.name}, ${t("gameprofToastPowerPlan")}: ${PLAN_LABELS[res.powerPlan] ?? res.powerPlan}${extra ? ` (${extra})` : ""}`;
        setLastResult({ ok: !res.warning && !res.error, text });
        showToast(text);
      } else {
        const text = `${t("gameprofToastError")}: ${res?.error ?? res?.warning ?? JSON.stringify(res)}`;
        setLastResult({ ok: false, text });
        showToast(text);
      }
    } catch (e: any) {
      const text = `${t("gameprofToastError")}: ${String(e)}`;
      setLastResult({ ok: false, text });
      showToast(text);
    }
    finally { await loadStatus(); setBusy(false); }
  };

  const revert = async () => {
    setBusy(true);
    try {
      const res = await api.gameRevert();
      if (res?.error) showToast(`${t("gameprofToastError")}: ${res.error}`);
      else showToast(`🔁 ${t("gameprofToastReverted")}${res?.note ? ` (${res.note})` : ""}`);
    } catch (e: any) { showToast(`${t("gameprofToastError")}: ${String(e)}`); }
    finally { await loadStatus(); setBusy(false); }
  };

  const currentPreset = (game: any) =>
    game.presets[preset] ?? game.presets.performance;

  return (
    <div className="page">
      {toast && (
        <div style={{
          position:"fixed", top:16, right:16, zIndex:9999,
          background:"var(--card2)", border:"1px solid var(--border)",
          borderRadius:8, padding:"10px 16px", color:"var(--text)",
          fontSize:13, boxShadow:"0 8px 24px rgba(0,0,0,.5)", maxWidth:360,
        }}>{toast}</div>
      )}

      <h1>{t("gameprofTitle")}</h1>
      <p className="sub">
        {t("gameprofSub")}
      </p>
      <ErrorBanner msg={loadErr} onRetry={loadAll} />

      {/* ── Status bar ─────────────────────────────────────────────────── */}
      <div style={{
        display:"flex", gap:12, alignItems:"center", flexWrap:"wrap",
        background:"var(--card)", border:"1px solid var(--border)",
        borderRadius:10, padding:"14px 18px", marginBottom:24,
      }}>
        <div style={{flex:1}}>
          <div style={{fontSize:12, color:"var(--muted)", marginBottom:4}}>{t("gameprofAutoSwitcher")}</div>
          <div style={{display:"flex", alignItems:"center", gap:10}}>
            <button
              className={`btn small ${status?.enabled ? "accent" : "ghost"}`}
              onClick={toggleSwitcher}
            >
              {status?.enabled ? t("gameprofEnabled") : t("gameprofDisabled")}
            </button>
            {status?.activeGame && (
              <span style={{fontSize:12, color:"var(--green)"}}>
                🎮 {t("gameprofActiveLabel")}: {games.find(g => g.id === status.activeGame)?.name ?? status.activeGame}
              </span>
            )}
          </div>
        </div>

        <div>
          <div style={{fontSize:12, color:"var(--muted)", marginBottom:4}}>{t("gameprofDefaultPreset")}</div>
          <div style={{display:"flex", gap:6}}>
            {(["performance","balanced","quality"] as const).map(p => (
              <button
                key={p}
                className={`btn small ${(status?.defaultPreset ?? "performance") === p ? "accent" : "ghost"}`}
                onClick={() => changeDefaultPreset(p)}
                style={{ borderColor: PRESET_LABELS[p].color }}
              >
                {PRESET_LABELS[p].icon} {PRESET_LABELS[p].label}
              </button>
            ))}
          </div>
        </div>

        {status?.activeGame && (
          <button className="btn small ghost danger" onClick={revert} disabled={busy}>
            {t("gameprofRevertNow")}
          </button>
        )}
      </div>

      {/* ── Layout ─────────────────────────────────────────────────────── */}
      <div style={{display:"grid", gridTemplateColumns:"260px 1fr", gap:16, height:"calc(100vh - 260px)"}}>

        {/* Game list */}
        <div style={{display:"flex", flexDirection:"column", gap:8, overflowY:"auto"}}>
          <input
            className="search-input"
            placeholder={t("gameprofSearchPlaceholder")}
            value={filter}
            onChange={e => setFilter(e.target.value)}
            style={{marginBottom:4}}
          />
          {genres.filter(genre =>
            filtered.some((g:any) => g.genre === genre)
          ).map(genre => (
            <div key={genre}>
              <div style={{fontSize:10, color:"var(--muted)", textTransform:"uppercase", letterSpacing:1, padding:"4px 8px"}}>
                {genre}
              </div>
              {filtered.filter((g:any) => g.genre === genre).map((game:any) => (
                <button
                  key={game.id}
                  onClick={() => setSelected(game)}
                  style={{
                    width:"100%", textAlign:"left", background:"none",
                    border: selectedGame?.id === game.id
                      ? "1px solid var(--accent)"
                      : "1px solid transparent",
                    borderRadius:8, padding:"8px 12px", cursor:"pointer",
                    color: selectedGame?.id === game.id ? "var(--text)" : "var(--muted)",
                    display:"flex", alignItems:"center", justifyContent:"space-between",
                  }}
                >
                  <span style={{fontSize:13}}>{game.name}</span>
                  {status?.activeGame === game.id && (
                    <span style={{fontSize:10, color:"var(--green)"}}>● {t("gameprofActive")}</span>
                  )}
                  {game.competitive && (
                    <span style={{fontSize:10, color:"var(--yellow)"}}>⚡</span>
                  )}
                </button>
              ))}
            </div>
          ))}
        </div>

        {/* Detail panel */}
        {selectedGame ? (
          <div style={{overflowY:"auto", display:"flex", flexDirection:"column", gap:16}}>

            {/* Header */}
            <div style={{
              background:"var(--card)", border:"1px solid var(--border)",
              borderRadius:10, padding:"16px 20px",
            }}>
              <div style={{display:"flex", alignItems:"flex-start", justifyContent:"space-between", flexWrap:"wrap", gap:12}}>
                <div>
                  <h2 style={{margin:0, fontSize:20}}>{selectedGame.name}</h2>
                  <div style={{fontSize:12, color:"var(--muted)", marginTop:4}}>
                    {selectedGame.genre}
                    {selectedGame.competitive && ` · ${t("gameprofCompetitive")}`}
                    {" · "}
                    <span style={{color:"var(--muted)"}}>
                      {t("gameprofProcesses")}: {selectedGame.processes.join(", ")}
                    </span>
                  </div>
                </div>
                <div style={{display:"flex", gap:8}}>
                  {(["performance","balanced","quality"] as const).map(p => (
                    <button
                      key={p}
                      className={`btn small ${preset === p ? "accent" : "ghost"}`}
                      onClick={() => setPreset(p)}
                      style={{ borderColor: PRESET_LABELS[p].color }}
                    >
                      {PRESET_LABELS[p].icon} {PRESET_LABELS[p].label}
                    </button>
                  ))}
                  <button
                    className="btn small accent"
                    onClick={() => applyPreset(selectedGame, preset)}
                    disabled={busy}
                  >
                    {t("gameprofApplyNow")}
                  </button>
                </div>
              </div>

              {lastResult && (
                <div role={lastResult.ok ? "status" : "alert"} style={{ marginTop: 10, fontSize: 13, color: lastResult.ok ? "var(--green)" : "var(--red)" }}>
                  {lastResult.text}
                </div>
              )}

              {/* Preset description */}
              <div style={{
                marginTop:12, padding:"10px 14px",
                background:"var(--card2)", borderRadius:8,
                fontSize:13, color:"var(--muted)", borderLeft:`3px solid ${PRESET_LABELS[preset].color}`,
              }}>
                {currentPreset(selectedGame).description}
                <span style={{marginLeft:12, fontSize:12, color:"var(--muted)"}}>
                  {t("gameprofPowerLabel")}: {PLAN_LABELS[currentPreset(selectedGame).power_plan] ?? currentPreset(selectedGame).power_plan}
                </span>
              </div>
            </div>

            {/* Settings table */}
            <div style={{
              background:"var(--card)", border:"1px solid var(--border)",
              borderRadius:10, padding:"16px 20px",
            }}>
              <div style={{
                fontSize:11, color:"var(--muted)", textTransform:"uppercase",
                letterSpacing:1, marginBottom:12,
              }}>
                {t("gameprofRecommendedSettings")}, {PRESET_LABELS[preset].label}
              </div>

              {/* Group by category */}
              {(() => {
                const settings: any[] = currentPreset(selectedGame).settings ?? [];
                const cats = [...new Set(settings.map((s:any) => s.cat))];
                return cats.map((cat:string) => (
                  <div key={cat} style={{marginBottom:16}}>
                    <div style={{
                      fontSize:11, color:"var(--accent)", textTransform:"uppercase",
                      letterSpacing:1, marginBottom:6, fontWeight:600,
                    }}>{cat}</div>
                    <div style={{
                      borderRadius:8, overflow:"hidden",
                      border:"1px solid var(--border)",
                    }}>
                      {settings.filter((s:any) => s.cat === cat).map((s:any, i:number) => (
                        <div key={i} style={{
                          display:"flex", justifyContent:"space-between", alignItems:"center",
                          padding:"7px 12px", gap:16,
                          background: i % 2 === 0 ? "var(--card)" : "var(--card2)",
                          borderBottom: i < settings.filter((s:any) => s.cat === cat).length - 1
                            ? "1px solid var(--border)" : "none",
                        }}>
                          <span style={{fontSize:13, color:"var(--muted)", flex:1}}>{s.name}</span>
                          <span style={{
                            fontSize:13, color:"var(--text)", fontWeight:500,
                            textAlign:"right", maxWidth:280,
                          }}>{s.value}</span>
                        </div>
                      ))}
                    </div>
                  </div>
                ));
              })()}
            </div>

            {/* Tips */}
            {selectedGame.tips?.length > 0 && (
              <div style={{
                background:"var(--card)", border:"1px solid var(--border)",
                borderRadius:10, padding:"16px 20px",
              }}>
                <div style={{
                  fontSize:11, color:"var(--muted)", textTransform:"uppercase",
                  letterSpacing:1, marginBottom:12,
                }}>{t("gameprofProTips")}</div>
                {selectedGame.tips.map((tip: string, i: number) => (
                  <div key={i} style={{
                    display:"flex", gap:10, alignItems:"flex-start",
                    padding:"6px 0",
                    borderBottom: i < selectedGame.tips.length - 1 ? "1px solid var(--border)" : "none",
                  }}>
                    <span style={{color:"var(--yellow)", fontSize:12, marginTop:1}}>→</span>
                    <span style={{fontSize:13, color:"var(--muted)"}}>{tip}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ) : (
          <div style={{
            display:"flex", flexDirection:"column", alignItems:"center",
            justifyContent:"center", color:"var(--muted)", gap:12, height:"100%",
          }}>
            <div style={{fontSize:48}}>🎮</div>
            <div style={{fontSize:14}}>{t("gameprofSelectGame")}</div>
            <div style={{fontSize:12, color:"var(--muted)"}}>
              {games.length} {t("gameprofGamesInDb")}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
