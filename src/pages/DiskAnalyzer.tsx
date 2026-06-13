import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, fmtBytes } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";

// ── types ────────────────────────────────────────────────────────────────────

type Drive = {
  name: string; root: string;
  used: number; free: number; total: number; pct: number;
  usedFmt: string; freeFmt: string; totFmt: string;
};
type FileEntry   = { path: string; name: string; size: number; sizeFmt: string; ext: string; modified: number };
type FolderEntry = { path: string; name: string; relPath: string; size: number; sizeFmt: string; pct: number };
type LargestData = { files: FileEntry[]; folders: FolderEntry[]; fileCount: number; totalSize: number; totalSizeFmt: string; capped: boolean };
type DupFile     = { path: string; name: string; modified: number };
type DupGroup    = { hash: string; size: number; sizeFmt: string; count: number; wasted: number; wastedFmt: string; files: DupFile[] };
type DupData     = { groups: DupGroup[]; totalWasted: number; totalWastedFmt: string; checked: number };
type TempBucket  = { label: string; count: number; size: number; sizeFmt: string; pct: number };
type TempData    = { buckets: TempBucket[]; totalCount: number; totalSize: number; totalSizeFmt: string; dirs: string[] };
type Tab         = "largest" | "duplicates" | "temp" | "organizer";

// ── helpers ───────────────────────────────────────────────────────────────────

const ts = (s: number) => s ? new Date(s * 1000).toLocaleDateString("de-DE") : "–";

function BarMini({ pct, color = "var(--accent)" }: { pct: number; color?: string }) {
  return (
    <div style={{ height: 4, background: "var(--border)", borderRadius: 2, width: "100%", marginTop: 4 }}>
      <div style={{ height: 4, borderRadius: 2, width: `${Math.min(pct, 100)}%`, background: color }} />
    </div>
  );
}

// ── Action bar ────────────────────────────────────────────────────────────────

type ActionBarProps = {
  selection: Set<string>;
  selectionSize: number; // total bytes of selected items
  drives: Drive[];
  currentRoot: string;
  onClear: () => void;
  onDeleted: () => void;
  onMoved: () => void;
};

function ActionBar({ selection, selectionSize, drives, currentRoot, onClear, onDeleted, onMoved }: ActionBarProps) {
  const { t } = useLang();
  const [confirm, setConfirm] = useState<"delete" | "move" | null>(null);
  const [moveDest, setMoveDest] = useState("");
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<string | null>(null);

  const otherDrives = drives.filter(d => d.root !== currentRoot);

  const doDelete = async () => {
    setBusy(true);
    try {
      const r = await api.diskDelete([...selection]);
      setResult(`✔ ${r.deleted} ${t("diskDeleted")}${r.failed ? `, ${r.failed} ${t("diskErrors")}` : ""}`);
      onDeleted();
      onClear();
    } catch (e: any) {
      setResult(`✘ ${e}`);
    } finally {
      setBusy(false);
      setConfirm(null);
    }
  };

  const doMove = async () => {
    if (!moveDest) return;
    setBusy(true);
    try {
      const r = await api.diskMove([...selection], moveDest);
      setResult(`✔ ${r.moved} ${t("diskMoved")}${r.failed ? `, ${r.failed} ${t("diskErrors")}` : ""}`);
      onMoved();
      onClear();
    } catch (e: any) {
      setResult(`✘ ${e}`);
    } finally {
      setBusy(false);
      setConfirm(null);
    }
  };

  if (selection.size === 0 && !result) return null;

  return (
    <div style={{
      position: "sticky", bottom: 0,
      background: "var(--card)",
      border: "1px solid var(--accent)55",
      borderRadius: 10,
      padding: "10px 16px",
      marginTop: 12,
      display: "flex", alignItems: "center", gap: 10, flexWrap: "wrap",
      boxShadow: "0 -4px 20px #00000055",
      zIndex: 100,
    }}>
      {result && !busy ? (
        <>
          <span style={{ color: result.startsWith("✔") ? "var(--green)" : "var(--red)", fontWeight: 600 }}>
            {result}
          </span>
          <button className="btn small ghost" onClick={() => { setResult(null); onClear(); }}>{t("close")}</button>
        </>
      ) : confirm === "delete" ? (
        <>
          <span style={{ color: "var(--red)", fontWeight: 600 }}>
            ⚠ {selection.size} {t("diskItems")} ({fmtBytes(selectionSize)}) {t("diskConfirmDelete")}
          </span>
          <button className="btn small" style={{ background: "var(--red)" }} onClick={doDelete} disabled={busy}>
            {busy ? <Spinner /> : t("diskYesDelete")}
          </button>
          <button className="btn small ghost" onClick={() => setConfirm(null)} disabled={busy}>{t("cancel")}</button>
        </>
      ) : confirm === "move" ? (
        <>
          <span style={{ fontWeight: 600 }}>{selection.size} {t("diskMoveTo")}</span>
          <select
            value={moveDest}
            onChange={e => setMoveDest(e.target.value)}
            style={{ background: "var(--input-bg)", border: "1px solid var(--border)", color: "var(--text)", borderRadius: 6, padding: "4px 8px", fontSize: 12 }}
          >
            <option value="">{t("diskSelectDrive")}</option>
            {otherDrives.map(d => (
              <option key={d.root} value={d.root}>{d.name}: ({d.freeFmt} {t("diskDriveFree")})</option>
            ))}
            <option value="custom">{t("diskCustomPath")}</option>
          </select>
          {moveDest === "custom" && (
            <CustomPathInput onChange={setMoveDest} />
          )}
          <button className="btn small" onClick={doMove} disabled={busy || !moveDest || moveDest === "custom"}>
            {busy ? <Spinner /> : t("diskDoMove")}
          </button>
          <button className="btn small ghost" onClick={() => setConfirm(null)} disabled={busy}>{t("cancel")}</button>
        </>
      ) : (
        <>
          <span style={{ fontWeight: 600, color: "var(--accent)" }}>
            {selection.size} {t("diskSelected")} &nbsp;·&nbsp; {fmtBytes(selectionSize)}
          </span>
          <button className="btn small ghost" style={{ borderColor: "var(--red)", color: "var(--red)" }}
            onClick={() => setConfirm("delete")}>
            {t("diskDelete")}
          </button>
          <button className="btn small ghost" onClick={() => { setConfirm("move"); setMoveDest(""); }}>
            {t("diskMove")}
          </button>
          <button className="btn small ghost" onClick={onClear} style={{ marginLeft: "auto" }}>
            {t("diskClearSel")}
          </button>
        </>
      )}
    </div>
  );
}

function CustomPathInput({ onChange }: { onChange: (v: string) => void }) {
  const { t } = useLang();
  const [val, setVal] = useState("");
  return (
    <input
      placeholder={t("diskPathPlaceholder")}
      value={val}
      onChange={e => { setVal(e.target.value); onChange(e.target.value); }}
      style={{ background: "var(--input-bg)", border: "1px solid var(--border)", borderRadius: 6, color: "var(--text)", padding: "4px 10px", fontSize: 12, width: 180 }}
    />
  );
}

// ── Drive picker ──────────────────────────────────────────────────────────────

function DriveCard({ drive, selected, onClick }: { drive: Drive; selected: boolean; onClick: () => void }) {
  const { t } = useLang();
  const color = drive.pct > 90 ? "var(--red)" : drive.pct > 70 ? "var(--yellow)" : "var(--accent)";
  return (
    <div onClick={onClick} style={{
      border: `1px solid ${selected ? "var(--accent)" : "var(--border)"}`,
      borderRadius: 10, padding: "12px 16px", cursor: "pointer", minWidth: 140,
      background: selected ? "var(--accent)11" : "var(--card)", transition: "border-color .15s",
    }}>
      <div style={{ fontSize: 22, fontWeight: 800, color: selected ? "var(--accent)" : undefined }}>{drive.name}:</div>
      <div className="muted" style={{ fontSize: 11, marginTop: 2 }}>{drive.totFmt} {t("diskDriveTotal")}</div>
      <BarMini pct={drive.pct} color={color} />
      <div style={{ fontSize: 11, marginTop: 4, display: "flex", justifyContent: "space-between" }}>
        <span style={{ color }}>▪ {drive.usedFmt} {t("diskDriveUsed")}</span>
        <span className="muted">{drive.freeFmt} {t("diskDriveFree")}</span>
      </div>
    </div>
  );
}

// ── Checkbox ──────────────────────────────────────────────────────────────────

function Chk({ checked, onChange, indeterminate = false }: { checked: boolean; onChange: () => void; indeterminate?: boolean }) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => { if (ref.current) ref.current.indeterminate = indeterminate; }, [indeterminate]);
  return (
    <input
      ref={ref}
      type="checkbox"
      checked={checked}
      onChange={onChange}
      style={{ cursor: "pointer", accentColor: "var(--accent)", width: 14, height: 14, flexShrink: 0 }}
    />
  );
}

// ── Largest tab ───────────────────────────────────────────────────────────────

type LargestTabProps = { path: string; drives: Drive[]; currentRoot: string };

function LargestTab({ path, drives, currentRoot }: LargestTabProps) {
  const { t, lang } = useLang();
  const [data, setData]       = useState<LargestData | null>(null);
  const [loading, setLoading] = useState(false);
  const [view, setView]       = useState<"files" | "folders">("files");
  const [filter, setFilter]   = useState("");
  const [sel, setSel]         = useState<Set<string>>(new Set());

  const load = useCallback(() => {
    setLoading(true);
    setSel(new Set());
    api.diskLargest(path, 50).then(setData).finally(() => setLoading(false));
  }, [path]);

  useEffect(() => { load(); }, [load]);

  const filteredFiles = useMemo(() =>
    (data?.files ?? []).filter(f => !filter || f.name.toLowerCase().includes(filter.toLowerCase()) || f.ext.toLowerCase().includes(filter.toLowerCase())),
    [data, filter]);

  const filteredFolders = useMemo(() =>
    (data?.folders ?? []).filter(f => !filter || f.relPath.toLowerCase().includes(filter.toLowerCase())),
    [data, filter]);

  const items = view === "files" ? filteredFiles : filteredFolders;
  const itemPaths = items.map(i => i.path);
  const allSel  = itemPaths.length > 0 && itemPaths.every(p => sel.has(p));
  const someSel = !allSel && itemPaths.some(p => sel.has(p));

  const toggle = (path: string) =>
    setSel(s => { const n = new Set(s); n.has(path) ? n.delete(path) : n.add(path); return n; });

  const toggleAll = () =>
    setSel(allSel ? new Set() : new Set(itemPaths));

  // Compute total selected size
  const selSize = useMemo(() => {
    const allItems = [...(data?.files ?? []), ...(data?.folders ?? [])];
    return allItems.filter(f => sel.has(f.path)).reduce((acc, f) => acc + f.size, 0);
  }, [sel, data]);

  return (
    <div>
      {data && (
        <div style={{ display: "flex", gap: 14, flexWrap: "wrap", marginBottom: 14 }}>
          {[[t("diskFiles"), data.fileCount.toLocaleString(lang === "de" ? "de-DE" : "en-US")], [t("diskTotalSize"), data.totalSizeFmt], ["Ø", fmtBytes(data.fileCount > 0 ? data.totalSize / data.fileCount : 0)]].map(([l, v]) => (
            <div key={l} className="stat-chip"><div className="chip-val">{v}</div><div className="chip-lbl">{l}</div></div>
          ))}
          {data.capped && <div style={{ color: "var(--yellow)", fontSize: 12, alignSelf: "center" }}>⚠ Scan capped at 2M files</div>}
        </div>
      )}

      <div style={{ display: "flex", gap: 8, marginBottom: 12, flexWrap: "wrap", alignItems: "center" }}>
        <button className={`btn small ${view === "files" ? "" : "ghost"}`} onClick={() => { setView("files"); setSel(new Set()); }}>{t("diskFiles")}</button>
        <button className={`btn small ${view === "folders" ? "" : "ghost"}`} onClick={() => { setView("folders"); setSel(new Set()); }}>{t("diskFolders")}</button>
        <input placeholder="Filter…" value={filter} onChange={e => setFilter(e.target.value)}
          style={{ background: "var(--input-bg)", border: "1px solid var(--border)", borderRadius: 6, color: "var(--text)", padding: "4px 10px", fontSize: 12, width: 180 }} />
        <button className="btn small ghost" onClick={load} disabled={loading}>{loading ? <Spinner /> : "↻"}</button>
        {sel.size > 0 && <span style={{ color: "var(--accent)", fontSize: 12, marginLeft: 4 }}>{sel.size} {t("diskSelected")}</span>}
      </div>

      {loading && !data && <><Spinner /> <span className="muted">{t("diskLoading")}</span></>}

      {/* Files table */}
      {!loading && view === "files" && data && (
        <div style={{ overflowY: "auto", maxHeight: 420 }}>
          <table style={{ width: "100%", borderCollapse: "collapse", fontSize: 12 }}>
            <thead>
              <tr style={{ color: "var(--muted)", borderBottom: "1px solid var(--border)" }}>
                <th style={{ padding: "4px 8px", width: 20 }}>
                  <Chk checked={allSel} indeterminate={someSel} onChange={toggleAll} />
                </th>
                <th style={{ textAlign: "left", padding: "4px 8px" }}>#</th>
                <th style={{ textAlign: "left", padding: "4px 8px" }}>{t("colApp")}</th>
                <th style={{ textAlign: "left", padding: "4px 8px" }}>Type</th>
                <th style={{ textAlign: "right", padding: "4px 8px" }}>{t("colSize")}</th>
                <th style={{ textAlign: "right", padding: "4px 8px" }}>{t("colDate")}</th>
              </tr>
            </thead>
            <tbody>
              {filteredFiles.map((f, i) => {
                const checked = sel.has(f.path);
                return (
                  <tr key={f.path} title={f.path}
                    style={{ borderBottom: "1px solid var(--border-subtle, #282828)", background: checked ? "var(--accent)0d" : undefined, cursor: "pointer" }}
                    onClick={() => toggle(f.path)}>
                    <td style={{ padding: "5px 8px" }}><Chk checked={checked} onChange={() => toggle(f.path)} /></td>
                    <td style={{ padding: "5px 8px", color: "var(--muted)" }}>{i + 1}</td>
                    <td style={{ padding: "5px 8px", maxWidth: 300, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {f.name}
                      <div className="mono muted" style={{ fontSize: 10, marginTop: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{f.path}</div>
                    </td>
                    <td style={{ padding: "5px 8px" }}>
                      {f.ext && <span style={{ background: "var(--card2, #2a2a2a)", borderRadius: 4, padding: "1px 6px", fontSize: 10 }}>{f.ext}</span>}
                    </td>
                    <td style={{ padding: "5px 8px", textAlign: "right", fontWeight: 600 }}>{f.sizeFmt}</td>
                    <td style={{ padding: "5px 8px", textAlign: "right", color: "var(--muted)" }}>{ts(f.modified)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {/* Folders list */}
      {!loading && view === "folders" && data && (
        <div style={{ overflowY: "auto", maxHeight: 420 }}>
          {/* Select all header */}
          <div style={{ display: "flex", alignItems: "center", gap: 10, padding: "4px 10px", borderBottom: "1px solid var(--border)", color: "var(--muted)", fontSize: 11 }}>
            <Chk checked={allSel} indeterminate={someSel} onChange={toggleAll} />
            <span>{t("diskSelectCopies")}</span>
          </div>
          {filteredFolders.map((f, i) => {
            const checked = sel.has(f.path);
            return (
              <div key={f.path} title={f.path} onClick={() => toggle(f.path)} style={{
                display: "flex", alignItems: "center", gap: 12,
                padding: "8px 10px", borderBottom: "1px solid var(--border-subtle, #282828)",
                background: checked ? "var(--accent)0d" : undefined, cursor: "pointer",
              }}>
                <Chk checked={checked} onChange={() => toggle(f.path)} />
                <span style={{ color: "var(--muted)", width: 24, textAlign: "right", fontSize: 11 }}>{i + 1}</span>
                <div style={{ flex: 1, overflow: "hidden" }}>
                  <div style={{ fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{f.name}</div>
                  <div className="mono muted" style={{ fontSize: 10, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>…\{f.relPath}</div>
                  <BarMini pct={f.pct} />
                </div>
                <div style={{ textAlign: "right", flexShrink: 0 }}>
                  <div style={{ fontWeight: 700 }}>{f.sizeFmt}</div>
                  <div className="muted" style={{ fontSize: 10 }}>{f.pct}%</div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      <ActionBar
        selection={sel}
        selectionSize={selSize}
        drives={drives}
        currentRoot={currentRoot}
        onClear={() => setSel(new Set())}
        onDeleted={load}
        onMoved={load}
      />
    </div>
  );
}

// ── Duplicates tab ────────────────────────────────────────────────────────────

type DuplicatesTabProps = { path: string; drives: Drive[]; currentRoot: string };

function DuplicatesTab({ path, drives, currentRoot }: DuplicatesTabProps) {
  const { t, lang } = useLang();
  const [data, setData]       = useState<DupData | null>(null);
  const [loading, setLoading] = useState(false);
  const [open, setOpen]       = useState<string | null>(null);
  const [sel, setSel]         = useState<Set<string>>(new Set()); // selected file paths

  const load = () => { setLoading(true); setSel(new Set()); api.diskDuplicates(path).then(setData).finally(() => setLoading(false)); };

  const toggle = (p: string) => setSel(s => { const n = new Set(s); n.has(p) ? n.delete(p) : n.add(p); return n; });

  // Auto-select all non-first (copies) in a group
  const selectAllCopies = () => {
    const copies = (data?.groups ?? []).flatMap(g => g.files.slice(1).map(f => f.path));
    setSel(new Set(copies));
  };

  const selSize = useMemo(() => {
    if (!data) return 0;
    return data.groups.flatMap(g => g.files).filter(f => sel.has(f.path)).reduce((acc, f) => acc + (data.groups.find(g => g.files.some(ff => ff.path === f.path))?.size ?? 0), 0);
  }, [sel, data]);

  return (
    <div>
      {!data && !loading && (
        <div style={{ textAlign: "center", padding: "40px 0" }}>
          <div className="muted" style={{ marginBottom: 14 }}>SHA-256 · Min. 100 KB · Only size-matched candidates are hashed</div>
          <button className="btn" onClick={load}>🔍 {t("diskScanDupes")}</button>
        </div>
      )}
      {loading && <><Spinner /> <span className="muted">Hashing candidates…</span></>}
      {data && !loading && (
        <>
          <div style={{ display: "flex", gap: 14, flexWrap: "wrap", marginBottom: 14, alignItems: "center" }}>
            {[[t("diskDupesTitle"), data.groups.length], [t("diskDupesWasted"), data.totalWastedFmt], [t("diskDupesChecked"), data.checked.toLocaleString(lang === "de" ? "de-DE" : "en-US")]].map(([l, v]) => (
              <div key={String(l)} className="stat-chip"><div className="chip-val">{v}</div><div className="chip-lbl">{l}</div></div>
            ))}
            <button className="btn small ghost" onClick={selectAllCopies} style={{ marginLeft: 4 }}>{t("diskSelectCopies")}</button>
            <button className="btn small ghost" onClick={load}>↻ Rescan</button>
          </div>

          {data.groups.length === 0 && <div className="muted" style={{ textAlign: "center", padding: 30 }}>✓ No duplicates (≥100 KB)</div>}

          {data.groups.map(g => (
            <div key={g.hash} style={{ border: "1px solid var(--border)", borderRadius: 8, marginBottom: 8, overflow: "hidden" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "10px 14px", cursor: "pointer", background: open === g.hash ? "var(--card2, #222)" : undefined }}
                onClick={() => setOpen(open === g.hash ? null : g.hash)}>
                <span style={{ fontWeight: 700, color: "var(--red)" }}>×{g.count}</span>
                <div style={{ flex: 1 }}>
                  <div style={{ fontWeight: 600 }}>{g.files[0]?.name}</div>
                  <div className="mono muted" style={{ fontSize: 10 }}>hash: {g.hash}</div>
                </div>
                <div style={{ textAlign: "right" }}>
                  <div style={{ fontWeight: 700 }}>{g.sizeFmt}/file</div>
                  <div style={{ color: "var(--red)", fontSize: 11 }}>↓ {g.wastedFmt} wasted</div>
                </div>
                <span className="muted">{open === g.hash ? "▲" : "▼"}</span>
              </div>

              {open === g.hash && (
                <div style={{ borderTop: "1px solid var(--border)", padding: "8px 14px" }}>
                  {g.files.map((f, i) => {
                    const checked = sel.has(f.path);
                    return (
                      <div key={f.path} onClick={() => toggle(f.path)} style={{
                        display: "flex", alignItems: "center", gap: 10, padding: "6px 0",
                        borderBottom: i < g.files.length - 1 ? "1px solid var(--border-subtle, #282828)" : undefined,
                        background: checked ? "var(--accent)0d" : undefined, cursor: "pointer", borderRadius: 4,
                      }}>
                        <Chk checked={checked} onChange={() => toggle(f.path)} />
                        <span style={{
                          background: i === 0 ? "var(--accent)22" : "var(--red)22",
                          color: i === 0 ? "var(--accent)" : "var(--red)",
                          borderRadius: 4, padding: "1px 6px", fontSize: 10, flexShrink: 0,
                        }}>{i === 0 ? "original" : "copy"}</span>
                        <div className="mono" style={{ fontSize: 11, flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }} title={f.path}>{f.path}</div>
                        <div className="muted" style={{ fontSize: 10, flexShrink: 0 }}>{ts(f.modified)}</div>
                      </div>
                    );
                  })}
                </div>
              )}
            </div>
          ))}

          <ActionBar
            selection={sel}
            selectionSize={selSize}
            drives={drives}
            currentRoot={currentRoot}
            onClear={() => setSel(new Set())}
            onDeleted={load}
            onMoved={load}
          />
        </>
      )}
    </div>
  );
}

// ── Temp age tab ──────────────────────────────────────────────────────────────

const BUCKET_COLORS = ["var(--accent)", "#22b8cf", "var(--yellow)", "#f76707", "var(--red)"];

function TempTab() {
  const { t, lang } = useLang();
  const [data, setData]       = useState<TempData | null>(null);
  const [loading, setLoading] = useState(true);
  const load = () => { setLoading(true); api.diskTempAge().then(setData).finally(() => setLoading(false)); };
  useEffect(() => { load(); }, []);

  if (loading) return <><Spinner /> <span className="muted">{t("diskScanTemp")}</span></>;
  if (!data) return null;

  return (
    <div>
      <div style={{ display: "flex", gap: 14, flexWrap: "wrap", marginBottom: 14 }}>
        {[[t("diskTempTitle"), data.totalCount.toLocaleString(lang === "de" ? "de-DE" : "en-US")], [t("diskTotalSize"), data.totalSizeFmt]].map(([l, v]) => (
          <div key={String(l)} className="stat-chip"><div className="chip-val">{v}</div><div className="chip-lbl">{l}</div></div>
        ))}
        <button className="btn small ghost" onClick={load} style={{ alignSelf: "center" }}>↻</button>
      </div>
      <div className="muted" style={{ fontSize: 11, marginBottom: 14 }}>
        {t("diskTempDirs")}: {data.dirs.join(" · ")}
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {data.buckets.map((b, i) => (
          <div key={b.label}>
            <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 4, fontSize: 13 }}>
              <span style={{ fontWeight: 500 }}>{b.label}</span>
              <span><b>{b.count.toLocaleString(lang === "de" ? "de-DE" : "en-US")}</b><span className="muted"> {t("diskFiles2")} · </span><b style={{ color: BUCKET_COLORS[i] }}>{b.sizeFmt}</b></span>
            </div>
            <div style={{ height: 8, background: "var(--border)", borderRadius: 4 }}>
              <div style={{ height: 8, borderRadius: 4, width: `${Math.min(b.pct, 100)}%`, background: BUCKET_COLORS[i], transition: "width .4s" }} />
            </div>
          </div>
        ))}
      </div>
      <div className="muted" style={{ fontSize: 11, marginTop: 16 }}>
        ℹ To clean: go to the Cleanup page → select "Temp Files".
      </div>
    </div>
  );
}

// ── Organizer tab ─────────────────────────────────────────────────────────────

const CAT_COLORS: Record<string, string> = {
  "Images":      "#3b9edd",
  "Videos":      "#9b59b6",
  "Music":       "#e74c3c",
  "Documents":   "#27ae60",
  "Archives":    "#e67e22",
  "Code":        "#1abc9c",
  "Executables": "#e91e63",
  "Fonts":       "#795548",
  "3D & CAD":    "#607d8b",
  "Torrents":    "#009688",
};

type OrgItem = { src: string; dest: string; name: string; size: number; sizeFmt: string; cat: string; catFolder: string; icon: string };
type OrgPreview = { items: OrgItem[]; uncategorized: any[]; uncatCount: number; totalFiles: number };

function OrganizerTab({ defaultFolder }: { defaultFolder: string }) {
  const [folder, setFolder]     = useState(defaultFolder);
  const [recurse, setRecurse]   = useState(false);
  const [preview, setPreview]   = useState<OrgPreview | null>(null);
  const [loading, setLoading]   = useState(false);
  const [applying, setApplying] = useState(false);
  const [result, setResult]     = useState<any>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());

  const scan = async () => {
    if (!folder.trim()) return;
    setLoading(true); setPreview(null); setResult(null); setSelected(new Set());
    try { setPreview(await api.diskOrganizePreview(folder.trim(), recurse)); }
    finally { setLoading(false); }
  };

  const apply = async () => {
    if (!preview) return;
    const items = preview.items.filter(i => selected.has(i.src));
    if (!items.length) return;
    setApplying(true); setResult(null);
    try {
      const r = await api.diskOrganizeApply(items);
      setResult(r);
      // Re-scan to show updated state
      const fresh = await api.diskOrganizePreview(folder.trim(), recurse);
      setPreview(fresh);
      setSelected(new Set());
    } catch (e: any) { setResult({ error: String(e) }); }
    finally { setApplying(false); }
  };

  // Group preview items by category
  const grouped = useMemo(() => {
    if (!preview) return {} as Record<string, OrgItem[]>;
    return preview.items.reduce((acc, item) => {
      (acc[item.cat] ??= []).push(item);
      return acc;
    }, {} as Record<string, OrgItem[]>);
  }, [preview]);

  const toggleCat = (cat: string) => {
    const catItems = grouped[cat] ?? [];
    const allSel = catItems.every(i => selected.has(i.src));
    setSelected(prev => {
      const n = new Set(prev);
      catItems.forEach(i => allSel ? n.delete(i.src) : n.add(i.src));
      return n;
    });
  };

  const toggleItem = (src: string) => setSelected(prev => {
    const n = new Set(prev); n.has(src) ? n.delete(src) : n.add(src); return n;
  });

  const selectAll   = () => setSelected(new Set(preview?.items.map(i => i.src) ?? []));
  const deselectAll = () => setSelected(new Set());

  const COMMON_FOLDERS = [
    { label: "Downloads", path: (p: string) => p.replace(/^[A-Z]:\\/, (m) => m) },
  ];

  return (
    <div>
      {/* Folder picker */}
      <div style={{ display: "flex", gap: 8, marginBottom: 12, flexWrap: "wrap", alignItems: "center" }}>
        <input
          value={folder}
          onChange={e => setFolder(e.target.value)}
          placeholder="C:\Users\YourName\Downloads"
          style={{ flex: 1, minWidth: 220, background: "var(--input-bg)", border: "1px solid var(--border)", borderRadius: 6, color: "var(--text)", padding: "6px 10px", fontSize: 12 }}
        />
        <label style={{ display: "flex", alignItems: "center", gap: 5, fontSize: 12, cursor: "pointer", userSelect: "none" }}>
          <input type="checkbox" checked={recurse} onChange={e => setRecurse(e.target.checked)} style={{ accentColor: "var(--accent)" }} />
          Include subfolders
        </label>
        <button className="btn small" onClick={scan} disabled={loading || !folder.trim()}>
          {loading ? <><Spinner /> Scanning…</> : "🔍 Scan"}
        </button>
      </div>

      {/* Quick path buttons */}
      <div style={{ display: "flex", gap: 6, marginBottom: 14, flexWrap: "wrap" }}>
        {[
          { label: "🏠 Desktop", sub: "Desktop" },
          { label: "⬇ Downloads", sub: "Downloads" },
          { label: "📄 Documents", sub: "Documents" },
          { label: "🖼 Pictures", sub: "Pictures" },
          { label: "🎵 Music", sub: "Music" },
          { label: "🎬 Videos", sub: "Videos" },
        ].map(({ label, sub }) => (
          <button key={sub} className="btn small ghost" onClick={() => {
            const base = folder.match(/^[A-Z]:\\Users\\[^\\]+/)?.[0]
              ?? "C:\\Users\\" + (folder.split("\\")[2] ?? "User");
            setFolder(`${base}\\${sub}`);
          }}>{label}</button>
        ))}
      </div>

      {/* Result banner */}
      {result && (
        <div style={{
          marginBottom: 12, padding: "10px 14px", borderRadius: 6, fontSize: 13,
          background: result.error ? "rgba(255,80,80,0.08)" : "rgba(80,200,120,0.08)",
          border: `1px solid ${result.error ? "var(--red)" : "var(--green)"}`,
          color: result.error ? "var(--red)" : "var(--green)",
        }}>
          {result.error ? `✗ ${result.error}` : `✓ Moved ${result.moved} files${result.failed ? ` · ${result.failed} failed` : ""}`}
        </div>
      )}

      {preview && (
        <>
          {/* Summary + action bar */}
          <div style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap", marginBottom: 14, padding: "10px 14px", borderRadius: 8, background: "var(--card)", border: "1px solid var(--border)" }}>
            <span style={{ fontSize: 13 }}>
              <b style={{ color: "var(--accent)" }}>{preview.totalFiles}</b>
              <span className="muted"> files to sort</span>
              {preview.uncatCount > 0 && <span className="muted"> · {preview.uncatCount} unrecognized (skipped)</span>}
            </span>
            <div style={{ marginLeft: "auto", display: "flex", gap: 6 }}>
              <button className="btn small ghost" onClick={selectAll}>Select all</button>
              <button className="btn small ghost" onClick={deselectAll}>Clear</button>
              <button className="btn small" disabled={!selected.size || applying} onClick={apply}>
                {applying ? <><Spinner /> Moving…</> : `📁 Move (${selected.size})`}
              </button>
            </div>
          </div>

          {preview.totalFiles === 0 && (
            <div className="muted" style={{ textAlign: "center", padding: 30 }}>
              ✓ No unsorted files found in this folder.
            </div>
          )}

          {/* Category groups */}
          {Object.entries(grouped).map(([cat, items]) => {
            const catColor = CAT_COLORS[cat] ?? "var(--accent)";
            const catIcon  = items[0]?.icon ?? "📁";
            const allSel   = items.every(i => selected.has(i.src));
            const someSel  = !allSel && items.some(i => selected.has(i.src));
            const catSize  = items.reduce((a, i) => a + i.size, 0);

            return (
              <div key={cat} style={{ marginBottom: 10, border: "1px solid var(--border)", borderRadius: 8, overflow: "hidden" }}>
                {/* Category header */}
                <div
                  onClick={() => toggleCat(cat)}
                  style={{
                    display: "flex", alignItems: "center", gap: 10, padding: "10px 14px",
                    background: `${catColor}11`, borderBottom: "1px solid var(--border)",
                    cursor: "pointer", userSelect: "none",
                  }}
                >
                  <input
                    type="checkbox"
                    checked={allSel}
                    ref={el => { if (el) el.indeterminate = someSel; }}
                    onChange={() => toggleCat(cat)}
                    onClick={e => e.stopPropagation()}
                    style={{ accentColor: catColor, width: 14, height: 14 }}
                  />
                  <span style={{ fontSize: 18 }}>{catIcon}</span>
                  <span style={{ fontWeight: 700, fontSize: 13, color: catColor }}>{cat}</span>
                  <span className="muted" style={{ fontSize: 11 }}>→ {items[0]?.catFolder}\</span>
                  <span style={{ marginLeft: "auto", fontSize: 11, color: catColor }}>{items.length} files · {fmtBytes(catSize)}</span>
                </div>

                {/* File list */}
                <div style={{ maxHeight: 200, overflowY: "auto" }}>
                  {items.map(item => (
                    <div
                      key={item.src}
                      onClick={() => toggleItem(item.src)}
                      style={{
                        display: "flex", alignItems: "center", gap: 10, padding: "6px 14px",
                        borderBottom: "1px solid var(--border-subtle, #282828)",
                        background: selected.has(item.src) ? `${catColor}0d` : undefined,
                        cursor: "pointer",
                      }}
                    >
                      <input
                        type="checkbox"
                        checked={selected.has(item.src)}
                        onChange={() => toggleItem(item.src)}
                        onClick={e => e.stopPropagation()}
                        style={{ accentColor: catColor, width: 13, height: 13, flexShrink: 0 }}
                      />
                      <div style={{ flex: 1, overflow: "hidden" }}>
                        <div style={{ fontSize: 12, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{item.name}</div>
                        <div className="mono muted" style={{ fontSize: 10, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{item.src}</div>
                      </div>
                      <span style={{ fontSize: 11, fontWeight: 600, flexShrink: 0, color: "var(--muted)" }}>{item.sizeFmt}</span>
                    </div>
                  ))}
                </div>
              </div>
            );
          })}

          {/* Unrecognized files */}
          {preview.uncatCount > 0 && (
            <div style={{ marginTop: 10, padding: "8px 14px", borderRadius: 6, background: "var(--bg2)", border: "1px solid var(--border)" }}>
              <div className="muted" style={{ fontSize: 12 }}>
                ⚠ {preview.uncatCount} unrecognized file{preview.uncatCount !== 1 ? "s" : ""} (unknown extension) — not moved.
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────

export default function DiskAnalyzer() {
  const { t } = useLang();
  const [drives, setDrives]             = useState<Drive[]>([]);
  const [drivesLoading, setDrivesLoading] = useState(true);
  const [selected, setSelected]         = useState<Drive | null>(null);
  const [tab, setTab]                   = useState<Tab>("largest");

  const refreshDrives = () => {
    setDrivesLoading(true);
    api.diskDrives().then((r: any) => {
      const list: Drive[] = r.drives ?? [];
      setDrives(list);
      if (!selected && list.length > 0) setSelected(list[0]);
    }).finally(() => setDrivesLoading(false));
  };

  useEffect(() => { refreshDrives(); }, []);

  const TAB_LABELS: { id: Tab; label: string }[] = [
    { id: "largest",    label: `⬛ ${t("diskTabLargest")}` },
    { id: "duplicates", label: `⬛ ${t("diskTabDupes")}` },
    { id: "temp",       label: `⬛ ${t("diskTabTemp")}` },
    { id: "organizer",  label: "📁 File Organizer" },
  ];

  return (
    <>
      <div className="page-title">{t("diskTitle")}</div>
      <div className="page-sub">{t("diskSub")}</div>

      <Card title="Drives">
        {drivesLoading && <><Spinner /> <span className="muted">{t("diskLoading")}</span></>}
        {!drivesLoading && drives.length === 0 && <div className="muted">{t("diskNoDrives")}</div>}
        <div style={{ display: "flex", gap: 10, flexWrap: "wrap" }}>
          {drives.map(d => (
            <DriveCard key={d.root} drive={d} selected={selected?.root === d.root} onClick={() => { setSelected(d); setTab("largest"); }} />
          ))}
        </div>
      </Card>

      {selected && (
        <div className="mt">
          <Card title={`${t("diskAnalyzingDrive")}: ${selected.root}`}>
            <div style={{ display: "flex", gap: 6, marginBottom: 16, flexWrap: "wrap" }}>
              {TAB_LABELS.map(tl => (
                <button key={tl.id} className={`btn small ${tab === tl.id ? "" : "ghost"}`} onClick={() => setTab(tl.id)}>{tl.label}</button>
              ))}
            </div>
            {tab === "largest"    && <LargestTab    key={selected.root} path={selected.root} drives={drives} currentRoot={selected.root} />}
            {tab === "duplicates" && <DuplicatesTab key={selected.root} path={selected.root} drives={drives} currentRoot={selected.root} />}
            {tab === "temp"       && <TempTab />}
            {tab === "organizer"  && <OrganizerTab  key={selected.root} defaultFolder={selected.root} />}
          </Card>
        </div>
      )}
    </>
  );
}
