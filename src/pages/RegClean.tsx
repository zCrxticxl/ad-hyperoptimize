import React, { useEffect, useState, useMemo } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";

type Orphan = {
  id: string;
  category: string;
  root: string;
  keyPath: string;
  valueName: string;
  relatedValues: string[];
  displayName: string;
  badPath: string;
  reason: string;
};

type ScanData = { orphans: Orphan[]; total: number; counts: Record<string, number> };

const CATEGORY_COLORS: Record<string, string> = {
  "MUI Cache":       "var(--cyan)",
  "Uninstall":       "var(--yellow)",
  "App Paths":       "#a78bfa",
  "SharedDLLs":      "#fb923c",
  "Shell Extensions":"#f472b6",
  "Autostart":       "var(--red)",
};

const CAT_ORDER = ["MUI Cache", "Uninstall", "App Paths", "SharedDLLs", "Shell Extensions", "Autostart"];

export default function RegClean({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData] = useState<ScanData | null>(null);
  const [scanning, setScanning] = useState(false);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [filter, setFilter] = useState<string>("All");
  const [search, setSearch] = useState("");
  const [cleaning, setCleaning] = useState(false);
  const [confirmClean, setConfirmClean] = useState(false);
  const [result, setResult] = useState<any | null>(null);
  const [err, setErr] = useState("");
  const [backups, setBackups] = useState<any[] | null>(null);
  const [restoring, setRestoring] = useState<string | null>(null);
  const [restoreMsg, setRestoreMsg] = useState("");

  const doScan = () => {
    setScanning(true);
    setData(null);
    setSel(new Set());
    setResult(null);
    setErr("");
    api.regcleanScan()
      .then(setData)
      .catch((e) => setErr(String(e)))
      .finally(() => setScanning(false));
  };

  useEffect(() => { doScan(); }, []);

  const visible = useMemo<Orphan[]>(() => {
    if (!data) return [];
    let list = data.orphans;
    if (filter !== "All") list = list.filter((o) => o.category === filter);
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter(
        (o) => o.displayName.toLowerCase().includes(q) || o.badPath.toLowerCase().includes(q)
      );
    }
    return list;
  }, [data, filter, search]);

  const toggleOne = (id: string) =>
    setSel((s) => { const n = new Set(s); n.has(id) ? n.delete(id) : n.add(id); return n; });

  const selectAll  = () => setSel(new Set(visible.map((o) => o.id)));
  const selectNone = () => setSel(new Set());
  const selectCat  = (cat: string) =>
    setSel((s) => {
      const n = new Set(s);
      visible.filter((o) => o.category === cat).forEach((o) => n.add(o.id));
      return n;
    });

  const doClean = async () => {
    if (!data || sel.size === 0) return;
    const entries = data.orphans.filter((o) => sel.has(o.id));
    setCleaning(true);
    setErr("");
    setConfirmClean(false);
    try {
      const r = await api.regcleanClean(entries);
      setResult(r);
      await doScan();
    } catch (e: any) {
      setErr(String(e));
    } finally {
      setCleaning(false);
    }
  };

  const toggleBackups = async () => {
    if (backups) { setBackups(null); return; }
    setRestoreMsg("");
    try { setBackups(await api.regcleanListBackups()); }
    catch (e: any) { setErr(String(e)); }
  };

  const doRestore = async (path: string) => {
    setRestoring(path);
    setRestoreMsg("");
    try {
      const r = await api.regcleanRestore(path);
      setRestoreMsg(`✓ ${r.restored} restored${r.errors?.length ? ` · ${r.errors.length} errors` : ""}`);
      await doScan();
    } catch (e: any) {
      setRestoreMsg(String(e));
    } finally {
      setRestoring(null);
    }
  };

  const allCats = data
    ? CAT_ORDER.filter((c) => (data.counts[c] ?? 0) > 0).concat(
        Object.keys(data.counts).filter((c) => !CAT_ORDER.includes(c))
      )
    : [];

  return (
    <>
      <h1 className="page-title">{t("regTitle")}</h1>
      <div className="page-sub">
        {t("regSub")}
      </div>

      {/* Scan button + status */}
      <div style={{ display: "flex", gap: 10, marginBottom: 16, alignItems: "center", flexWrap: "wrap" }}>
        <button className="btn" onClick={doScan} disabled={scanning || cleaning}>
          {scanning ? <><Spinner /> {t("regScanning")}</> : t("regScan")}
        </button>
        <button className="btn ghost" onClick={toggleBackups} disabled={cleaning}>
          ↩ {backups ? t("regHideRestore") : t("regRestoreBtn")}
        </button>
        {data && (
          <span className="muted" style={{ fontSize: 13 }}>
            {data.total} {t("regFound")}
          </span>
        )}
        {err && <span style={{ color: "var(--red)", fontSize: 13 }}>{err}</span>}
      </div>

      {/* Restore panel */}
      {backups && (
        <Card title={t("regRestoreTitle")} style={{ marginBottom: 16 }}>
          {restoreMsg && <div style={{ fontSize: 13, marginBottom: 8, color: restoreMsg.startsWith("✓") ? "var(--green)" : "var(--red)" }}>{restoreMsg}</div>}
          {backups.length === 0 ? (
            <div className="muted" style={{ fontSize: 13 }}>{t("regNoBackups")}</div>
          ) : (
            <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
              {backups.map((b) => (
                <div key={b.path} className="row" style={{ gap: 10, alignItems: "center", padding: "6px 0", borderBottom: "1px solid var(--border)" }}>
                  <span style={{ fontSize: 13, flex: 1 }}>
                    {new Date(b.time).toLocaleString()} · <span className="muted">{b.count} {t("regEntriesWord")}</span>
                  </span>
                  <button className="btn ghost small" onClick={() => api.openPath(b.path)}>{t("open")}</button>
                  <button className="btn small" disabled={restoring === b.path} onClick={() => doRestore(b.path)}>
                    {restoring === b.path ? <><Spinner /> {t("regRestoring")}</> : t("regRestore")}
                  </button>
                </div>
              ))}
            </div>
          )}
        </Card>
      )}

      {/* Result banner */}
      {result && (
        <div
          style={{
            background: "var(--bg2)",
            border: "1px solid var(--green)",
            borderRadius: 8,
            padding: "10px 14px",
            marginBottom: 16,
            fontSize: 13,
          }}
        >
          <span style={{ color: "var(--green)", fontWeight: 700 }}>
            ✓ {result.deleted} {t("regCleaned")}
          </span>{" "}
          {t("regBackupAt")}{" "}
          <span className="mono" style={{ fontSize: 11 }}>{result.backupPath || "-"}</span>
          {result.backupPath && (
            <button
              className="btn ghost small"
              style={{ marginLeft: 10 }}
              onClick={() => api.openPath(result.backupPath)}
            >
              {t("open")}
            </button>
          )}
          {result.errors?.length > 0 && (
            <div style={{ color: "var(--yellow)", marginTop: 6 }}>
              ⚠ {result.errors.length} {t("regErrors")} {result.errors.slice(0, 3).join(" · ")}
              {result.errors.length > 3 ? ` … +${result.errors.length - 3}` : ""}
            </div>
          )}
        </div>
      )}

      {scanning && <><Spinner /> <span className="muted">{t("regScanningReg")}</span></>}

      {data && (
        <>
          {/* Summary chips */}
          <div style={{ display: "flex", gap: 10, marginBottom: 16, flexWrap: "wrap" }}>
            {allCats.map((cat) => (
              <button
                key={cat}
                className="stat-chip"
                type="button"
                aria-pressed={filter === cat || filter === "All"}
                style={{
                  borderColor: CATEGORY_COLORS[cat] ?? "var(--border)",
                  cursor: "pointer",
                  font: "inherit",
                  opacity: filter === cat || filter === "All" ? 1 : 0.5,
                }}
                onClick={() => setFilter(filter === cat ? "All" : cat)}
              >
                <span className="chip-val" style={{ color: CATEGORY_COLORS[cat] ?? "var(--fg)", fontSize: 20 }}>
                  {data.counts[cat] ?? 0}
                </span>
                <span className="chip-lbl">{cat}</span>
              </button>
            ))}
          </div>

          {/* Toolbar */}
          <div style={{ display: "flex", gap: 8, marginBottom: 12, alignItems: "center", flexWrap: "wrap" }}>
            <button className="btn ghost small" onClick={selectAll}>{t("selectAll")}</button>
            <button className="btn ghost small" onClick={selectNone}>{t("selectNone")}</button>
            {filter !== "All" && (
              <button className="btn ghost small" onClick={() => selectCat(filter)}>
                {t("selectAll")} {filter}
              </button>
            )}
            <input
              type="text"
              placeholder={t("search")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              style={{
                marginLeft: "auto",
                background: "var(--bg2)",
                border: "1px solid var(--border)",
                borderRadius: 6,
                padding: "4px 10px",
                color: "var(--fg)",
                fontSize: 13,
                width: 220,
              }}
            />
          </div>

          <Card title={`${visible.length} ${t("regEntries")} · ${sel.size} ${t("diskSelected")}`}>
            <table className="tbl">
              <thead>
                <tr>
                  <th scope="col" style={{ width: 28 }}>
                    <input
                      type="checkbox"
                      checked={sel.size > 0 && visible.every((o) => sel.has(o.id))}
                      onChange={(e) => (e.target.checked ? selectAll() : selectNone())}
                    />
                  </th>
                  <th scope="col">{t("regColName")}</th>
                  <th scope="col">{t("regColCat")}</th>
                  <th scope="col">{t("regColPath")}</th>
                  <th scope="col">{t("regColReason")}</th>
                </tr>
              </thead>
              <tbody>
                {visible.map((o) => (
                  <tr key={o.id} style={{ opacity: sel.has(o.id) ? 1 : 0.7 }}>
                    <td>
                      <input
                        type="checkbox"
                        checked={sel.has(o.id)}
                        onChange={() => toggleOne(o.id)}
                      />
                    </td>
                    <td style={{ fontWeight: 600, maxWidth: 200, wordBreak: "break-word" }}>
                      {o.displayName}
                      {o.relatedValues.length > 1 && (
                        <span
                          className="muted"
                          style={{ fontSize: 10, marginLeft: 5 }}
                          title={o.relatedValues.join("\n")}
                        >
                          ×{o.relatedValues.length} {t("regValues")}
                        </span>
                      )}
                    </td>
                    <td>
                      <span
                        style={{
                          fontSize: 11,
                          fontWeight: 600,
                          color: CATEGORY_COLORS[o.category] ?? "var(--fg)",
                          background: "var(--bg2)",
                          borderRadius: 4,
                          padding: "1px 6px",
                          whiteSpace: "nowrap",
                        }}
                      >
                        {o.category}
                      </span>
                    </td>
                    <td
                      className="mono muted"
                      style={{ fontSize: 11, maxWidth: 280, wordBreak: "break-all" }}
                      title={o.badPath}
                    >
                      {o.badPath.length > 55 ? "…" + o.badPath.slice(-52) : o.badPath}
                    </td>
                    <td className="muted" style={{ fontSize: 12 }}>
                      {o.reason}
                    </td>
                  </tr>
                ))}
                {visible.length === 0 && (
                  <tr>
                    <td colSpan={5} className="muted" style={{ textAlign: "center", padding: "20px 0" }}>
                      {data.total === 0 ? t("regEmpty") : t("regNoFilter")}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>

            {/* Action footer */}
            <div style={{ display: "flex", alignItems: "center", gap: 12, marginTop: 14, flexWrap: "wrap" }}>
              {!confirmClean ? (
                <button
                  className="btn"
                  disabled={sel.size === 0 || cleaning}
                  onClick={() => setConfirmClean(true)}
                  style={sel.size > 0 ? { background: "var(--accent)" } : {}}
                >
                  {cleaning ? <><Spinner /> {t("regClean")}</> : `🗑 ${t("regBackupPrefix")} ${sel.size} ${t("regCleanEntries")}`}
                </button>
              ) : (
                <>
                  <span style={{ color: "var(--yellow)", fontSize: 13 }}>
                    ⚠ {t("regConfirmClean")} ({sel.size})
                  </span>
                  <button className="btn danger" disabled={cleaning} onClick={doClean}>
                    {cleaning ? <><Spinner /> {t("regClean")}</> : t("regConfirmBtn")}
                  </button>
                  <button className="btn small ghost" onClick={() => setConfirmClean(false)} disabled={cleaning}>{t("cancel")}</button>
                </>
              )}
              {!admin && (
                <span style={{ color: "var(--yellow)", fontSize: 12 }}>
                  {t("regAdminHint")}
                </span>
              )}
              <span className="muted" style={{ fontSize: 12, marginLeft: "auto" }}>
                {t("regBackupNote")}
              </span>
            </div>
          </Card>
        </>
      )}
    </>
  );
}
