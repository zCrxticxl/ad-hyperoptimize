import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";

export default function GameBooster({ admin }: { admin: boolean }) {
  const [games, setGames]       = useState<any[]>([]);
  const [bg, setBg]             = useState<any[]>([]);
  const [selBg, setSelBg]       = useState<Set<number>>(new Set());
  const [loading, setLoading]   = useState(true);
  const [boosted, setBoosted]   = useState<number | null>(null);
  const [busy, setBusy]         = useState(false);
  const [log, setLog]           = useState("");

  const refresh = async () => {
    setLoading(true);
    try {
      const [g, b] = await Promise.all([
        api.gameboostRunningGames(),
        api.gameboostBackgroundProcs(),
      ]);
      setGames(g.games ?? []);
      setBg(b.procs ?? []);
    } finally { setLoading(false); }
  };

  useEffect(() => { refresh(); }, []);

  const act = async (fn: () => Promise<string>) => {
    setBusy(true);
    try { setLog(await fn()); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(false); }
  };

  const toggleBg = (pid: number) => {
    setSelBg(prev => {
      const n = new Set(prev);
      if (n.has(pid)) n.delete(pid); else n.add(pid);
      return n;
    });
  };

  const selectAllBg = () => setSelBg(new Set(bg.map((p: any) => p.Id)));
  const clearSelBg  = () => setSelBg(new Set());

  return (
    <>
      <div className="page-title">🎮 Game Booster</div>
      <div className="page-sub">
        Boost your game's process priority, kill background bloat, and switch to max-perf power plan.
        Click "Stop Boost" when done to restore settings.
        {!admin && <span style={{ color: "var(--orange)" }}> · Admin recommended for power plan switch.</span>}
      </div>

      {/* Running games */}
      <Card title="Detected Games / Heavy Processes" style={{ marginBottom: 14 }}>
        {loading ? <Spinner /> : games.length === 0 ? (
          <div className="muted">No heavy foreground processes detected. Launch a game first, then refresh.</div>
        ) : games.map((g: any) => (
          <div key={g.Id} className="row" style={{ padding: "8px 0", borderBottom: "1px solid var(--border)", gap: 12, alignItems: "center" }}>
            <div style={{ flex: 1 }}>
              <div style={{ fontWeight: 600, fontSize: 13 }}>
                {g.Name}
                {boosted === g.Id && (
                  <span style={{ marginLeft: 8, fontSize: 11, color: "var(--green)", fontWeight: 700 }}>● BOOSTED</span>
                )}
              </div>
              <div className="muted" style={{ fontSize: 11 }}>
                PID {g.Id} · {g.memMb} MB RAM · {g.priority}
                {g.MainWindowTitle && ` · "${g.MainWindowTitle}"`}
              </div>
            </div>
            {boosted !== g.Id ? (
              <button
                className="btn small"
                disabled={busy}
                onClick={() => act(async () => {
                  const r = await api.gameboostStart(g.Id);
                  setBoosted(g.Id);
                  return r;
                })}
              >
                {busy ? <><Spinner /> …</> : "⚡ Boost"}
              </button>
            ) : (
              <button
                className="btn small ghost danger"
                disabled={busy}
                onClick={() => act(async () => {
                  const r = await api.gameboostStop();
                  setBoosted(null);
                  return r;
                })}
              >
                {busy ? "…" : "⏹ Stop Boost"}
              </button>
            )}
          </div>
        ))}
        <div className="row" style={{ marginTop: 10, gap: 8 }}>
          <button className="btn small ghost" onClick={refresh} disabled={loading}>↺ Refresh</button>
          {boosted !== null && (
            <button
              className="btn small ghost danger"
              disabled={busy}
              onClick={() => act(async () => {
                const r = await api.gameboostStop();
                setBoosted(null);
                return r;
              })}
            >
              ⏹ Stop All Boosts
            </button>
          )}
        </div>
      </Card>

      {/* Background killers */}
      <Card title={`Background Processes to Kill (${bg.length} found)`}>
        {loading ? <Spinner /> : bg.length === 0 ? (
          <div className="muted">No known background bloat running.</div>
        ) : (
          <>
            <div className="row" style={{ gap: 8, marginBottom: 10 }}>
              <button className="btn small ghost" onClick={selectAllBg}>Select all</button>
              <button className="btn small ghost" onClick={clearSelBg}>Clear</button>
              <div style={{ flex: 1 }} />
              <button
                className="btn small danger"
                disabled={!selBg.size || busy}
                onClick={() => act(async () => {
                  const r = await api.gameboostKillBackground([...selBg]);
                  setSelBg(new Set());
                  await refresh();
                  return r;
                })}
              >
                {busy ? "…" : `💀 Kill (${selBg.size})`}
              </button>
            </div>
            {bg.map((p: any) => (
              <label
                key={p.Id}
                className="row"
                style={{ gap: 10, padding: "5px 0", cursor: "pointer", borderBottom: "1px solid var(--border)", alignItems: "center" }}
              >
                <input
                  type="checkbox"
                  checked={selBg.has(p.Id)}
                  onChange={() => toggleBg(p.Id)}
                />
                <span style={{ flex: 1, fontSize: 13, fontWeight: 500 }}>{p.Name}</span>
                <span className="muted" style={{ fontSize: 11 }}>
                  {p.memMb > 0 ? `${p.memMb} MB RAM` : ""}
                </span>
              </label>
            ))}
          </>
        )}
      </Card>

      {log && (
        <div className="mono muted" style={{ fontSize: 12, marginTop: 12 }}>{log}</div>
      )}

      <div className="muted" style={{ fontSize: 12, marginTop: 14 }}>
        Boost: sets game to High priority + switches to max-perf power plan + silences notifications.
        Stop Boost: reverts power plan to Balanced and re-enables notifications.
        Killed processes must be manually restarted.
      </div>
    </>
  );
}
