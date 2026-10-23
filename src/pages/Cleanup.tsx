import React, { useEffect, useState } from "react";
import { api, fmtAge, fmtBytes } from "../api";
import { Card, Spinner, ActionBtn } from "../components/ui";
import { useLang } from "../i18n";

/** Categories under %WINDIR% / ProgramData: deleting there needs admin. */
const ADMIN_CATS = new Set(["win_temp", "wu_cache", "crash_dumps"]);

export default function Cleanup({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [cats, setCats] = useState<any[] | null>(null);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [result, setResult] = useState<any | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [err, setErr] = useState("");
  const [meta, setMeta] = useState<{ time: string; fromCache: boolean } | null>(null);
  const [scanning, setScanning] = useState(false);

  const scan = (force: boolean, keepResult = false) => {
    setScanning(true);
    if (!keepResult) { setCats(null); setResult(null); }
    setErr("");
    api.scanCleanup(force).then((env) => {
      setCats(env.data);
      setMeta({ time: env.time, fromCache: !!env.fromCache });
      setSel(new Set());
    }).catch((e: any) => { setErr(String(e)); setCats((c) => c ?? []); })
      .finally(() => setScanning(false));
  };
  useEffect(() => scan(false), []);
  const locked = (id: string) => !admin && ADMIN_CATS.has(id);

  const toggle = (id: string) =>
    setSel((s) => {
      const n = new Set(s);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  const selectedBytes = (cats ?? []).filter((c) => sel.has(c.id)).reduce((a, c) => a + c.bytes, 0);

  return (
    <>
      <h1 className="page-title">{t("cleanupTitle")}</h1>
      <div className="page-sub">
        {t("cleanupSub")}
      </div>

      {!cats && <><Spinner /> <span className="muted">{t("cleanupScanning")}</span></>}
      {meta && cats && (
        <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>
          {t("dataAsOf").replace("{age}", fmtAge(meta.time))}{meta.fromCache ? ` (${t("fromCacheTag")})` : ""}
        </div>
      )}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}

      {cats && (
        <Card title={`${t("cleanupReclaimable")}: ${fmtBytes(cats.reduce((a, c) => a + c.bytes, 0))}`}>
          <table className="tbl">
            <thead>
              <tr><th scope="col"></th><th scope="col">{t("cleanupColCategory")}</th><th scope="col">{t("cleanupColFiles")}</th><th scope="col">{t("cleanupColSize")}</th><th scope="col">{t("cleanupColNotes")}</th></tr>
            </thead>
            <tbody>
              {cats.map((c) => (
                <tr key={c.id}>
                  <td><input type="checkbox" checked={sel.has(c.id)} disabled={locked(c.id)} onChange={() => toggle(c.id)} aria-label={c.name} /></td>
                  <td>{c.name}{locked(c.id) && <span className="muted" style={{ fontSize: 11 }}> · {t("cleanupAdminCat")}</span>}</td>
                  <td>{c.fileCount}</td>
                  <td>{fmtBytes(c.bytes)}</td>
                  <td className="muted" style={{ maxWidth: 380 }}>{c.note}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <div className="row mt">
            {!confirm ? (
              <button className="btn" disabled={sel.size === 0} onClick={() => setConfirm(true)}>
                {t("cleanupCleanSelected")} ({fmtBytes(selectedBytes)})…
              </button>
            ) : (
              <>
                <span style={{ color: "var(--yellow)" }}>
                  {t("cleanupDeletePrefix")} {fmtBytes(selectedBytes)} {t("cleanupDeleteAcross")} {sel.size} {sel.size === 1 ? t("cleanupCategory") : t("cleanupCategories")}? {t("cleanupInUseSkipped")}
                </span>
                <ActionBtn
                  label={t("cleanupConfirmDelete")}
                  className="btn danger"
                  onRun={async () => {
                    const r = await api.runCleanup([...sel]);
                    setResult(r);
                    setConfirm(false);
                    setSel(new Set());
                    scan(true, true);
                  }}
                />
                <button className="btn ghost" onClick={() => setConfirm(false)}>{t("cancel")}</button>
              </>
            )}
            <button className="btn ghost" disabled={scanning} onClick={() => scan(true)}>{scanning ? <Spinner /> : t("cleanupRescan")}</button>
          </div>
          {result && (
            <div className="mt" style={{ color: "var(--green)" }}>
              ✔ {t("cleanupFreed")} {fmtBytes(result.freedBytes)} · {result.deleted} {t("cleanupFilesDeleted")} · {result.skippedInUse} {t("cleanupInUseFilesSkipped")}
            </div>
          )}
        </Card>
      )}
    </>
  );
}
