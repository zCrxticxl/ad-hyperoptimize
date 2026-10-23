import React, { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Badge, Card, Spinner } from "./ui";
import { useLang, TKey } from "../i18n";

type Run = {
  id: string; time: string; game: string; state: "before" | "after"; note: string; file: string;
  app: string | null; otherApps: string[];
  stats: { frames: number; seconds: number; avgFps: number; low1Fps: number; low01Fps: number; p99Ms: number; stdevMs: number };
};
type Metric = { metric: "avgFps" | "low1Fps" | "low01Fps"; verdict: "tooFewRuns" | "withinNoise" | "better" | "worse"; detail: any };

const VERDICT_CLS: Record<string, string> = { better: "st-applied", worse: "risk-High", withinNoise: "st-unknown", tooFewRuns: "st-partial" };

/** Real game measurements: imported frame-time captures, compared before and
 * after a change with run-to-run variation taken into account. */
export function FpsMeasure() {
  const { t } = useLang();
  const [runs, setRuns] = useState<Run[]>([]);
  const [game, setGame] = useState("");
  const [state, setState] = useState<"before" | "after">("before");
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const [selected, setSelected] = useState("");
  const [cmp, setCmp] = useState<Metric[] | null>(null);

  const load = async () => {
    const r = await api.fpsRuns().catch((e: unknown) => ({ runs: [], error: String(e) }));
    setRuns(r.runs ?? []);
    if (r.error) setMsg({ ok: false, text: r.error });
  };
  useEffect(() => { load(); }, []);

  const games = useMemo(() => [...new Set(runs.map((r) => r.game))], [runs]);
  const current = selected || games[0] || "";
  useEffect(() => {
    if (!current) { setCmp(null); return; }
    api.fpsCompare(current).then((c) => setCmp(c.metrics ?? null)).catch(() => setCmp(null));
  }, [current, runs]);

  const importFiles = async (files: FileList | null) => {
    if (!files?.length) return;
    if (!game.trim()) { setMsg({ ok: false, text: t("fpsNeedGame") }); return; }
    setBusy(true);
    setMsg(null);
    const errors: string[] = [];
    let ok = 0;
    for (const f of Array.from(files)) {
      try {
        await api.fpsImport(await f.text(), f.name, game.trim(), state, note);
        ok++;
      } catch (e) {
        errors.push(`${f.name}: ${String(e)}`);
      }
    }
    await load();
    setSelected(game.trim());
    setBusy(false);
    setMsg(errors.length
      ? { ok: false, text: `${t("fpsImported").replace("{n}", String(ok))} ${errors.join(" · ")}` }
      : { ok: true, text: t("fpsImported").replace("{n}", String(ok)) });
  };

  const remove = async (id: string) => {
    if (!window.confirm(t("fpsDeleteConfirm"))) return;
    await api.fpsDelete(id).catch((e: unknown) => setMsg({ ok: false, text: String(e) }));
    await load();
  };

  const shown = runs.filter((r) => r.game === current);
  const num = (x: number) => x.toLocaleString(undefined, { maximumFractionDigits: 1 });

  return (
    <Card title={t("fpsTitle")} style={{ marginBottom: 14 }}>
      <div className="muted" style={{ fontSize: 12.5, marginBottom: 6 }}>{t("fpsIntro")}</div>
      <ol className="muted" style={{ fontSize: 12.5, margin: "0 0 12px 18px", padding: 0 }}>
        <li>{t("fpsStep1")}</li>
        <li>{t("fpsStep2")}</li>
        <li>{t("fpsStep3")}</li>
      </ol>

      <div className="row" style={{ gap: 8, flexWrap: "wrap", alignItems: "center", marginBottom: 10 }}>
        <input list="fps-games" placeholder={t("fpsGamePlaceholder")} value={game} onChange={(e) => setGame(e.target.value)}
          style={{ minWidth: 200, padding: "5px 10px", fontSize: 13, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }} />
        <datalist id="fps-games">{games.map((g) => <option key={g} value={g} />)}</datalist>
        <button className={`btn small ${state === "before" ? "" : "ghost"}`} onClick={() => setState("before")}>{t("fpsBefore")}</button>
        <button className={`btn small ${state === "after" ? "" : "ghost"}`} onClick={() => setState("after")}>{t("fpsAfter")}</button>
        <input placeholder={t("fpsNotePlaceholder")} value={note} onChange={(e) => setNote(e.target.value)}
          style={{ flex: 1, minWidth: 160, padding: "5px 10px", fontSize: 13, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)" }} />
        <label className="btn small" style={{ cursor: busy ? "wait" : "pointer" }}>
          {busy ? <Spinner /> : t("fpsImport")}
          <input type="file" accept=".csv,text/csv" multiple hidden disabled={busy}
            onChange={(e) => { importFiles(e.target.files); e.target.value = ""; }} />
        </label>
      </div>
      {msg && <div style={{ fontSize: 12, marginBottom: 10, color: msg.ok ? "var(--green)" : "var(--red)" }}>{msg.text}</div>}

      {games.length === 0 ? (
        <div className="muted" style={{ fontSize: 12.5 }}>{t("fpsNoRuns")}</div>
      ) : (
        <>
          <div className="row" style={{ gap: 6, flexWrap: "wrap", marginBottom: 10 }}>
            {games.map((g) => (
              <button key={g} className={`btn small ${g === current ? "" : "ghost"}`} onClick={() => setSelected(g)}>{g}</button>
            ))}
          </div>

          {cmp && (
            <div style={{ marginBottom: 12 }}>
              {cmp.map((m) => (
                <div key={m.metric} className="finding-row">
                  <Badge cls={VERDICT_CLS[m.verdict]}>{t(`fpsVerdict_${m.verdict}` as TKey)}</Badge>
                  <div className="finding-copy">
                    <b>{t(`fpsMetric_${m.metric}` as TKey)}</b>
                    <span>
                      {m.verdict === "tooFewRuns"
                        ? t("fpsTooFew").replace("{before}", String(m.detail.runsBefore)).replace("{after}", String(m.detail.runsAfter))
                        : t("fpsCompareLine")
                            .replace("{before}", num(m.detail.before)).replace("{sdb}", num(m.detail.sdBefore))
                            .replace("{after}", num(m.detail.after)).replace("{sda}", num(m.detail.sdAfter))
                            .replace("{diff}", `${m.detail.diff >= 0 ? "+" : ""}${num(m.detail.diff)}`)
                            .replace("{pct}", `${m.detail.diffPct >= 0 ? "+" : ""}${num(m.detail.diffPct)}`)
                            .replace("{noise}", num(m.detail.noise))}
                    </span>
                  </div>
                </div>
              ))}
              <div className="muted" style={{ fontSize: 11.5, marginTop: 6 }}>{t("fpsMethod")}</div>
            </div>
          )}

          <table className="tbl">
            <thead><tr>
              <th scope="col">{t("benchColTime")}</th><th scope="col">{t("fpsColState")}</th>
              <th scope="col">{t("fpsMetric_avgFps")}</th><th scope="col">{t("fpsMetric_low1Fps")}</th><th scope="col">{t("fpsMetric_low01Fps")}</th>
              <th scope="col">{t("fpsColCapture")}</th><th scope="col" />
            </tr></thead>
            <tbody>
              {[...shown].reverse().map((r) => (
                <tr key={r.id}>
                  <td className="muted">{new Date(r.time).toLocaleString()}</td>
                  <td>{r.state === "before" ? t("fpsBefore") : t("fpsAfter")}{r.note ? <span className="muted"> · {r.note}</span> : null}</td>
                  <td><b>{num(r.stats.avgFps)}</b></td>
                  <td>{num(r.stats.low1Fps)}</td>
                  <td>{num(r.stats.low01Fps)}</td>
                  <td className="muted" title={r.otherApps.length ? `${t("fpsOtherApps")} ${r.otherApps.join(", ")}` : ""}>
                    {r.app ?? r.file} · {r.stats.frames} · {num(r.stats.seconds)} s
                  </td>
                  <td><button className="btn small ghost" onClick={() => remove(r.id)} aria-label={t("fpsDelete")}>✕</button></td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </Card>
  );
}
