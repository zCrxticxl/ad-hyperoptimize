import { useEffect, useMemo, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";

const PRESET_LABELS: Record<string, { label: string; color: string; icon: string }> = {
  performance: { label: "Performance",  color: "#f59e0b", icon: "⚡" },
  balanced:    { label: "Balanced",     color: "#3b82f6", icon: "⚖️" },
  quality:     { label: "Quality",      color: "#8b5cf6", icon: "✨" },
};

const PLAN_LABELS: Record<string, string> = {
  ultimate:         "Ultimate Performance",
  high_performance: "High Performance",
  balanced:         "Balanced",
};

export default function GameProfiles() {
  const [games, setGames]           = useState<any[]>([]);
  const [status, setStatus]         = useState<any>(null);
  const [selectedGame, setSelected] = useState<any>(null);
  const [preset, setPreset]         = useState<"performance"|"balanced"|"quality">("performance");
  const [filter, setFilter]         = useState("");
  const [busy, setBusy]             = useState(false);
  const [toast, setToast]           = useState<string | null>(null);
  const [activeEvt, setActiveEvt]   = useState<any>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 3500);
  };

  useEffect(() => {
    api.gameList().then(setGames);
    api.gameSwitcherStatus().then(setStatus);

    const unSub1 = listen("game-detected", (e: any) => {
      setActiveEvt(e.payload);
      setStatus((s: any) => ({ ...s, activeGame: e.payload.id }));
      showToast(`🎮 ${e.payload.name} detected — ${e.payload.preset} profile applied`);
    });
    const unSub2 = listen("game-exited", (e: any) => {
      setActiveEvt(null);
      setStatus((s: any) => ({ ...s, activeGame: null }));
      showToast("🔁 Game closed — power plan reverted");
    });
    return () => { unSub1.then(f => f()); unSub2.then(f => f()); };
  }, []);

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
    await api.gameSwitcherConfigure(next, status.defaultPreset ?? "performance");
    setStatus((s: any) => ({ ...s, enabled: next }));
    showToast(next ? "✅ Auto-switcher enabled" : "⏸ Auto-switcher disabled");
  };

  const changeDefaultPreset = async (p: string) => {
    if (!status) return;
    await api.gameSwitcherConfigure(status.enabled, p);
    setStatus((s: any) => ({ ...s, defaultPreset: p }));
  };

  const applyPreset = async (game: any, p: string) => {
    setBusy(true);
    try {
      const res = await api.gameApplyPreset(game.id, p);
      if (res.ok) showToast(`⚡ Applied ${p} preset for ${game.name} — power plan: ${PLAN_LABELS[res.powerPlan] ?? res.powerPlan}`);
      else showToast(`Error: ${res.error}`);
    } finally { setBusy(false); }
  };

  const revert = async () => {
    setBusy(true);
    await api.gameRevert();
    setBusy(false);
    showToast("🔁 Reverted to previous power plan");
  };

  const currentPreset = (game: any) =>
    game.presets[preset] ?? game.presets.performance;

  return (
    <div className="page">
      {toast && (
        <div style={{
          position:"fixed", top:16, right:16, zIndex:9999,
          background:"#1e293b", border:"1px solid #334155",
          borderRadius:8, padding:"10px 16px", color:"#f1f5f9",
          fontSize:13, boxShadow:"0 8px 24px #0008", maxWidth:360,
        }}>{toast}</div>
      )}

      <h1>Game Profiles</h1>
      <p className="sub">
        Per-game optimized settings. Auto-switcher applies your preset the moment a game is detected.
      </p>

      {/* ── Status bar ─────────────────────────────────────────────────── */}
      <div style={{
        display:"flex", gap:12, alignItems:"center", flexWrap:"wrap",
        background:"#0f172a", border:"1px solid #1e293b",
        borderRadius:10, padding:"14px 18px", marginBottom:24,
      }}>
        <div style={{flex:1}}>
          <div style={{fontSize:12, color:"#64748b", marginBottom:4}}>AUTO-SWITCHER</div>
          <div style={{display:"flex", alignItems:"center", gap:10}}>
            <button
              className={`btn small ${status?.enabled ? "accent" : "ghost"}`}
              onClick={toggleSwitcher}
            >
              {status?.enabled ? "Enabled" : "Disabled"}
            </button>
            {status?.activeGame && (
              <span style={{fontSize:12, color:"#22c55e"}}>
                🎮 Active: {games.find(g => g.id === status.activeGame)?.name ?? status.activeGame}
              </span>
            )}
          </div>
        </div>

        <div>
          <div style={{fontSize:12, color:"#64748b", marginBottom:4}}>DEFAULT PRESET</div>
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
            Revert Now
          </button>
        )}
      </div>

      {/* ── Layout ─────────────────────────────────────────────────────── */}
      <div style={{display:"grid", gridTemplateColumns:"260px 1fr", gap:16, height:"calc(100vh - 260px)"}}>

        {/* Game list */}
        <div style={{display:"flex", flexDirection:"column", gap:8, overflowY:"auto"}}>
          <input
            className="search-input"
            placeholder="Search games or genre…"
            value={filter}
            onChange={e => setFilter(e.target.value)}
            style={{marginBottom:4}}
          />
          {genres.filter(genre =>
            filtered.some((g:any) => g.genre === genre)
          ).map(genre => (
            <div key={genre}>
              <div style={{fontSize:10, color:"#475569", textTransform:"uppercase", letterSpacing:1, padding:"4px 8px"}}>
                {genre}
              </div>
              {filtered.filter((g:any) => g.genre === genre).map((game:any) => (
                <button
                  key={game.id}
                  onClick={() => setSelected(game)}
                  style={{
                    width:"100%", textAlign:"left", background:"none",
                    border: selectedGame?.id === game.id
                      ? "1px solid #3b82f6"
                      : "1px solid transparent",
                    borderRadius:8, padding:"8px 12px", cursor:"pointer",
                    color: selectedGame?.id === game.id ? "#f1f5f9" : "#94a3b8",
                    display:"flex", alignItems:"center", justifyContent:"space-between",
                  }}
                >
                  <span style={{fontSize:13}}>{game.name}</span>
                  {status?.activeGame === game.id && (
                    <span style={{fontSize:10, color:"#22c55e"}}>● ACTIVE</span>
                  )}
                  {game.competitive && (
                    <span style={{fontSize:10, color:"#f59e0b"}}>⚡</span>
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
              background:"#0f172a", border:"1px solid #1e293b",
              borderRadius:10, padding:"16px 20px",
            }}>
              <div style={{display:"flex", alignItems:"flex-start", justifyContent:"space-between", flexWrap:"wrap", gap:12}}>
                <div>
                  <h2 style={{margin:0, fontSize:20}}>{selectedGame.name}</h2>
                  <div style={{fontSize:12, color:"#64748b", marginTop:4}}>
                    {selectedGame.genre}
                    {selectedGame.competitive && " · Competitive"}
                    {" · "}
                    <span style={{color:"#475569"}}>
                      Processes: {selectedGame.processes.join(", ")}
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
                    Apply Now
                  </button>
                </div>
              </div>

              {/* Preset description */}
              <div style={{
                marginTop:12, padding:"10px 14px",
                background:"#1e293b", borderRadius:8,
                fontSize:13, color:"#94a3b8", borderLeft:`3px solid ${PRESET_LABELS[preset].color}`,
              }}>
                {currentPreset(selectedGame).description}
                <span style={{marginLeft:12, fontSize:12, color:"#475569"}}>
                  Power: {PLAN_LABELS[currentPreset(selectedGame).power_plan] ?? currentPreset(selectedGame).power_plan}
                </span>
              </div>
            </div>

            {/* Settings table */}
            <div style={{
              background:"#0f172a", border:"1px solid #1e293b",
              borderRadius:10, padding:"16px 20px",
            }}>
              <div style={{
                fontSize:11, color:"#475569", textTransform:"uppercase",
                letterSpacing:1, marginBottom:12,
              }}>
                Recommended In-Game Settings — {PRESET_LABELS[preset].label}
              </div>

              {/* Group by category */}
              {(() => {
                const settings: any[] = currentPreset(selectedGame).settings ?? [];
                const cats = [...new Set(settings.map((s:any) => s.cat))];
                return cats.map((cat:string) => (
                  <div key={cat} style={{marginBottom:16}}>
                    <div style={{
                      fontSize:11, color:"#3b82f6", textTransform:"uppercase",
                      letterSpacing:1, marginBottom:6, fontWeight:600,
                    }}>{cat}</div>
                    <div style={{
                      borderRadius:8, overflow:"hidden",
                      border:"1px solid #1e293b",
                    }}>
                      {settings.filter((s:any) => s.cat === cat).map((s:any, i:number) => (
                        <div key={i} style={{
                          display:"flex", justifyContent:"space-between", alignItems:"center",
                          padding:"7px 12px", gap:16,
                          background: i % 2 === 0 ? "#0f172a" : "#111827",
                          borderBottom: i < settings.filter((s:any) => s.cat === cat).length - 1
                            ? "1px solid #1e293b" : "none",
                        }}>
                          <span style={{fontSize:13, color:"#94a3b8", flex:1}}>{s.name}</span>
                          <span style={{
                            fontSize:13, color:"#f1f5f9", fontWeight:500,
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
                background:"#0f172a", border:"1px solid #1e293b",
                borderRadius:10, padding:"16px 20px",
              }}>
                <div style={{
                  fontSize:11, color:"#475569", textTransform:"uppercase",
                  letterSpacing:1, marginBottom:12,
                }}>Pro Tips</div>
                {selectedGame.tips.map((tip: string, i: number) => (
                  <div key={i} style={{
                    display:"flex", gap:10, alignItems:"flex-start",
                    padding:"6px 0",
                    borderBottom: i < selectedGame.tips.length - 1 ? "1px solid #1e293b" : "none",
                  }}>
                    <span style={{color:"#f59e0b", fontSize:12, marginTop:1}}>→</span>
                    <span style={{fontSize:13, color:"#94a3b8"}}>{tip}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        ) : (
          <div style={{
            display:"flex", flexDirection:"column", alignItems:"center",
            justifyContent:"center", color:"#334155", gap:12, height:"100%",
          }}>
            <div style={{fontSize:48}}>🎮</div>
            <div style={{fontSize:14}}>Select a game to view its optimized settings</div>
            <div style={{fontSize:12, color:"#1e293b"}}>
              {games.length} games in database
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
