import React, { useCallback, useEffect, useMemo, useState } from "react";
import { api, fmtBytes } from "../api";
import { Card, Spinner } from "./ui";
import { useLang, TKey } from "../i18n";

type OrgItem = { src: string; name: string; size: number; sizeFmt: string; cat: string; target: string; renamed: boolean };
type OrgCategory = { id: string; name: string; icon: string; count: number; size: number; sizeFmt: string };
type OrgPreview = {
  folder: string;
  byMonth: boolean;
  items: OrgItem[];
  categories: OrgCategory[];
  skipped: { name: string; reason: string }[];
  uncategorized: { name: string; size: number; sizeFmt: string }[];
};
type OrgRun = { id: string; time: string; folder: string; moved: number; undone: boolean };

const CAT_COLORS: Record<string, string> = {
  Images: "#3b9edd", Videos: "#9b59b6", Music: "#e74c3c", Documents: "#27ae60", Archives: "#e67e22",
  Code: "#1abc9c", Executables: "#e91e63", Fonts: "#795548", "3D": "#607d8b", Torrents: "#009688",
};

/** Sorts the top-level files of one folder into category folders. Every run
 * is logged by the backend and can be moved back from the history below. */
export function Organizer() {
  const { t } = useLang();
  const [folders, setFolders] = useState<{ key: string; path: string }[]>([]);
  const [root, setRoot] = useState("");
  const [byMonth, setByMonth] = useState(false);
  const [preview, setPreview] = useState<OrgPreview | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [open, setOpen] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string; details?: string[] } | null>(null);
  const [runs, setRuns] = useState<OrgRun[]>([]);
  const [undoBusy, setUndoBusy] = useState<string | null>(null);

  const loadRuns = useCallback(() => {
    api.organizerRuns().then((r) => setRuns(r.runs ?? [])).catch(() => setRuns([]));
  }, []);

  useEffect(() => {
    api.organizerFolders().then((r) => setFolders(r.folders ?? [])).catch(() => {});
    loadRuns();
  }, [loadRuns]);

  const loadPreview = async (folder: string, month: boolean) => {
    const p: OrgPreview = await api.organizerPreview(folder, month);
    setPreview(p);
    setSelected(new Set(p.items.map((i) => i.src)));
    setOpen(new Set());
  };

  const doPreview = async (folder = root, month = byMonth) => {
    if (!folder.trim()) return;
    setBusy(true);
    setMsg(null);
    try { await loadPreview(folder.trim(), month); }
    catch (e: any) { setPreview(null); setMsg({ ok: false, text: String(e) }); }
    finally { setBusy(false); }
  };

  const pickFolder = (path: string) => {
    setRoot(path);
    doPreview(path, byMonth);
  };

  const toggleMonth = (v: boolean) => {
    setByMonth(v);
    if (preview) doPreview(preview.folder, v);
  };

  const byCat = useMemo(() => {
    const m: Record<string, OrgItem[]> = {};
    for (const it of preview?.items ?? []) (m[it.cat] ??= []).push(it);
    return m;
  }, [preview]);

  const chosen = (preview?.items ?? []).filter((i) => selected.has(i.src));
  const chosenSize = chosen.reduce((a, i) => a + i.size, 0);

  const setCat = (cat: string, on: boolean) =>
    setSelected((prev) => {
      const next = new Set(prev);
      for (const it of byCat[cat] ?? []) on ? next.add(it.src) : next.delete(it.src);
      return next;
    });
  const toggleOne = (src: string) =>
    setSelected((prev) => {
      const next = new Set(prev);
      next.has(src) ? next.delete(src) : next.add(src);
      return next;
    });
  const toggleOpen = (cat: string) =>
    setOpen((prev) => {
      const next = new Set(prev);
      next.has(cat) ? next.delete(cat) : next.add(cat);
      return next;
    });

  const apply = async () => {
    if (!preview || chosen.length === 0) return;
    if (!window.confirm(t("orgConfirm").replace("{n}", String(chosen.length)).replace("{size}", fmtBytes(chosenSize)))) return;
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.organizerApply(preview.folder, preview.byMonth, chosen.map((i) => i.src));
      const failed = Number(r.failed ?? 0);
      setMsg({
        ok: failed === 0,
        text: t("orgResult").replace("{moved}", String(r.moved ?? 0)).replace("{failed}", String(failed)),
        details: Array.isArray(r.errors) ? r.errors.map(String) : [],
      });
      loadRuns();
      try { await loadPreview(preview.folder, preview.byMonth); } catch { setPreview(null); }
    } catch (e: any) {
      setMsg({ ok: false, text: String(e) });
    } finally { setBusy(false); }
  };

  const undo = async (run: OrgRun) => {
    if (!window.confirm(t("orgUndoConfirm").replace("{n}", String(run.moved)).replace("{folder}", run.folder))) return;
    setUndoBusy(run.id);
    setMsg(null);
    try {
      const r = await api.organizerUndo(run.id);
      const skipped: { name: string; reason: string }[] = r.skipped ?? [];
      setMsg({
        ok: skipped.length === 0,
        text: t("orgUndoResult").replace("{restored}", String(r.restored ?? 0)).replace("{skipped}", String(skipped.length)),
        details: skipped.map((s) => `${s.name}: ${t(`orgUndoSkip_${s.reason}` as TKey)}`),
      });
      loadRuns();
      if (preview && preview.folder === run.folder) {
        try { await loadPreview(preview.folder, preview.byMonth); } catch { /* message above stays */ }
      }
    } catch (e: any) {
      setMsg({ ok: false, text: String(e) });
    } finally { setUndoBusy(null); }
  };

  const inputStyle: React.CSSProperties = { flex: 1, minWidth: 220, padding: "7px 10px", borderRadius: 6, border: "1px solid var(--border)", background: "var(--bg2)", color: "var(--text)", fontSize: 13 };

  return (
    <div style={{ paddingTop: 8 }}>
      <div className="muted" style={{ fontSize: 12.5, marginBottom: 10 }}>{t("orgIntro")}</div>

      {folders.length > 0 && (
        <div className="row" style={{ gap: 6, flexWrap: "wrap", marginBottom: 8 }}>
          {folders.map((f) => (
            <button key={f.key} className={`btn small ${preview?.folder === f.path ? "" : "ghost"}`} disabled={busy} onClick={() => pickFolder(f.path)} title={f.path}>
              {f.key === "downloads" ? "⬇" : f.key === "desktop" ? "🖥" : "📁"} {t(`orgFolder_${f.key}` as TKey)}
            </button>
          ))}
        </div>
      )}

      <div className="row" style={{ gap: 8, marginBottom: 8, flexWrap: "wrap" }}>
        <input value={root} onChange={(e) => setRoot(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") doPreview(); }} placeholder={t("diskOrganizeFolder")} style={inputStyle} />
        <button className="btn small" onClick={() => doPreview()} disabled={busy || !root.trim()}>
          {busy ? <Spinner /> : t("diskOrganizePreview")}
        </button>
      </div>
      <label className="row" style={{ gap: 6, fontSize: 12.5, marginBottom: 12, cursor: "pointer", alignItems: "center" }}>
        <input type="checkbox" checked={byMonth} onChange={(e) => toggleMonth(e.target.checked)} />
        {t("orgByMonth")}
      </label>

      {msg && (
        <div role={msg.ok ? "status" : "alert"} style={{ marginBottom: 10, padding: "8px 12px", borderRadius: 6, background: msg.ok ? "rgba(80,200,120,0.08)" : "rgba(255,97,125,.08)", border: `1px solid ${msg.ok ? "var(--green)" : "var(--red)"}`, color: msg.ok ? "var(--green)" : "var(--red)", fontSize: 13 }}>
          {msg.text}
          {msg.details && msg.details.length > 0 && (
            <ul className="mono" style={{ margin: "6px 0 0", paddingLeft: 18, fontSize: 11, maxHeight: 160, overflowY: "auto" }}>
              {msg.details.map((d, i) => <li key={i}>{d}</li>)}
            </ul>
          )}
        </div>
      )}

      {preview && (
        <Card title={t("orgPreviewTitle").replace("{folder}", preview.folder)}>
          {preview.items.length === 0 ? (
            <div className="muted" style={{ fontSize: 13 }}>{t("orgNothing")}</div>
          ) : (
            <>
              {preview.categories.map((c) => {
                const items = byCat[c.id] ?? [];
                const on = items.filter((i) => selected.has(i.src)).length;
                const color = CAT_COLORS[c.id] ?? "var(--accent)";
                return (
                  <div key={c.id} style={{ borderBottom: "1px solid var(--border)", padding: "6px 0" }}>
                    <div className="row" style={{ gap: 10, alignItems: "center" }}>
                      <input
                        type="checkbox"
                        checked={on === items.length}
                        ref={(el) => { if (el) el.indeterminate = on > 0 && on < items.length; }}
                        onChange={(e) => setCat(c.id, e.target.checked)}
                        aria-label={c.name}
                      />
                      <span style={{ fontSize: 16 }}>{c.icon}</span>
                      <b style={{ color, minWidth: 110 }}>{t(`orgCat_${c.id.replace(/\W/g, "")}` as TKey)}</b>
                      <span className="muted" style={{ fontSize: 12, flex: 1 }}>{on}/{c.count} · {c.sizeFmt}</span>
                      <button className="btn small ghost" onClick={() => toggleOpen(c.id)} aria-expanded={open.has(c.id)}>{open.has(c.id) ? "▲" : "▼"}</button>
                    </div>
                    {open.has(c.id) && (
                      <div style={{ maxHeight: 260, overflowY: "auto", margin: "6px 0 0 26px" }}>
                        {items.map((it) => (
                          <label key={it.src} className="row" style={{ gap: 8, padding: "3px 0", alignItems: "center", cursor: "pointer" }}>
                            <input type="checkbox" checked={selected.has(it.src)} onChange={() => toggleOne(it.src)} />
                            <span style={{ flex: 1, minWidth: 0, fontSize: 12.5, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={it.name}>{it.name}</span>
                            <span className="muted mono" style={{ fontSize: 11, maxWidth: "45%", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={it.target}>→ {it.target}</span>
                            {it.renamed && <span className="badge st-partial" title={t("orgRenamedHint")}>{t("orgRenamed")}</span>}
                            <span className="muted" style={{ fontSize: 11, minWidth: 64, textAlign: "right" }}>{it.sizeFmt}</span>
                          </label>
                        ))}
                      </div>
                    )}
                  </div>
                );
              })}
              <div className="row" style={{ marginTop: 12, gap: 10, alignItems: "center", flexWrap: "wrap" }}>
                <span className="muted" style={{ fontSize: 13, flex: 1 }}>
                  {t("orgSelected").replace("{n}", String(chosen.length)).replace("{size}", fmtBytes(chosenSize))}
                </span>
                <button className="btn small ghost" onClick={() => setSelected(new Set(chosen.length === preview.items.length ? [] : preview.items.map((i) => i.src)))}>
                  {chosen.length === preview.items.length ? t("diskDeselectAll") : t("diskSelectAll")}
                </button>
                <button className="btn small" onClick={apply} disabled={busy || chosen.length === 0}>
                  {busy ? <Spinner /> : `${t("diskMoveBtn")} (${chosen.length})`}
                </button>
              </div>
            </>
          )}

          {preview.skipped.length > 0 && (
            <details style={{ marginTop: 12 }}>
              <summary className="muted" style={{ fontSize: 12.5, cursor: "pointer" }}>{t("orgSkippedTitle").replace("{n}", String(preview.skipped.length))}</summary>
              <ul style={{ margin: "6px 0 0", paddingLeft: 18, fontSize: 12 }}>
                {preview.skipped.map((s) => <li key={s.name}><span className="mono">{s.name}</span>: <span className="muted">{t(`orgSkip_${s.reason}` as TKey)}</span></li>)}
              </ul>
            </details>
          )}
          {preview.uncategorized.length > 0 && (
            <details style={{ marginTop: 8 }}>
              <summary className="muted" style={{ fontSize: 12.5, cursor: "pointer" }}>{t("orgUncatTitle").replace("{n}", String(preview.uncategorized.length))}</summary>
              <ul style={{ margin: "6px 0 0", paddingLeft: 18, fontSize: 12, maxHeight: 200, overflowY: "auto" }}>
                {preview.uncategorized.map((u) => <li key={u.name}><span className="mono">{u.name}</span> <span className="muted">{u.sizeFmt}</span></li>)}
              </ul>
            </details>
          )}
        </Card>
      )}

      <Card title={t("orgHistoryTitle")} style={{ marginTop: 14 }}>
        {runs.length === 0 ? (
          <div className="muted" style={{ fontSize: 13 }}>{t("orgHistoryEmpty")}</div>
        ) : runs.map((r) => (
          <div key={r.id} className="row" style={{ gap: 10, padding: "6px 0", borderBottom: "1px solid var(--border)", alignItems: "center" }}>
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 13 }}>{new Date(r.time).toLocaleString()} · {t("orgRunMoved").replace("{n}", String(r.moved))}</div>
              <div className="muted mono" style={{ fontSize: 11, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={r.folder}>{r.folder}</div>
            </div>
            {r.undone ? (
              <span className="muted" style={{ fontSize: 12 }}>{t("orgRunUndone")}</span>
            ) : (
              <button className="btn small ghost" disabled={undoBusy !== null || r.moved === 0} onClick={() => undo(r)}>
                {undoBusy === r.id ? <Spinner /> : `↩ ${t("orgUndo")}`}
              </button>
            )}
          </div>
        ))}
        <div className="muted" style={{ fontSize: 11.5, marginTop: 8 }}>{t("orgHistoryHint")}</div>
      </Card>
    </div>
  );
}
