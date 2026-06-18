import React, { useEffect, useState } from "react";
import { api, fmtBytes } from "../api";
import { Card, Spinner, ActionBtn } from "../components/ui";
import { useLang } from "../i18n";

export default function Cleanup() {
  const { t } = useLang();
  const [cats, setCats] = useState<any[] | null>(null);
  const [sel, setSel] = useState<Set<string>>(new Set());
  const [result, setResult] = useState<any | null>(null);
  const [confirm, setConfirm] = useState(false);

  const scan = (force: boolean) => {
    setCats(null);
    setResult(null);
    api.scanCleanup(force).then((env) => {
      setCats(env.data);
      setSel(new Set());
    });
  };
  useEffect(() => scan(false), []);

  const toggle = (id: string) =>
    setSel((s) => {
      const n = new Set(s);
      n.has(id) ? n.delete(id) : n.add(id);
      return n;
    });

  const selectedBytes = (cats ?? []).filter((c) => sel.has(c.id)).reduce((a, c) => a + c.bytes, 0);

  return (
    <>
      <div className="page-title">{t("cleanupTitle")}</div>
      <div className="page-sub">
        {t("cleanupSub")}
      </div>

      {!cats && <><Spinner /> <span className="muted">{t("cleanupScanning")}</span></>}

      {cats && (
        <Card title={`${t("cleanupReclaimable")}: ${fmtBytes(cats.reduce((a, c) => a + c.bytes, 0))}`}>
          <table className="tbl">
            <thead>
              <tr><th></th><th>{t("cleanupColCategory")}</th><th>{t("cleanupColFiles")}</th><th>{t("cleanupColSize")}</th><th>{t("cleanupColNotes")}</th></tr>
            </thead>
            <tbody>
              {cats.map((c) => (
                <tr key={c.id}>
                  <td><input type="checkbox" checked={sel.has(c.id)} onChange={() => toggle(c.id)} /></td>
                  <td>{c.name}</td>
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
                    api.scanCleanup(true).then((env) => setCats(env.data));
                    setSel(new Set());
                  }}
                />
                <button className="btn ghost" onClick={() => setConfirm(false)}>{t("cancel")}</button>
              </>
            )}
            <button className="btn ghost" onClick={() => scan(true)}>{t("cleanupRescan")}</button>
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
