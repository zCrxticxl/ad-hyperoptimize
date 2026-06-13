import React, { useEffect, useState } from "react";
import { api, fmtBytes } from "../api";
import { Card, Spinner, ActionBtn } from "../components/ui";

export default function Cleanup() {
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
      <div className="page-title">Cleanup</div>
      <div className="page-sub">
        Scan is read-only. Deletion only touches the whitelisted cache/temp folders you tick below — in-use files are always skipped, never forced.
      </div>

      {!cats && <><Spinner /> <span className="muted">Scanning cleanup targets…</span></>}

      {cats && (
        <Card title={`Reclaimable: ${fmtBytes(cats.reduce((a, c) => a + c.bytes, 0))}`}>
          <table className="tbl">
            <thead>
              <tr><th></th><th>Category</th><th>Files</th><th>Size</th><th>Notes</th></tr>
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
                Clean selected ({fmtBytes(selectedBytes)})…
              </button>
            ) : (
              <>
                <span style={{ color: "var(--yellow)" }}>
                  Delete {fmtBytes(selectedBytes)} across {sel.size} categor{sel.size === 1 ? "y" : "ies"}? In-use files are skipped.
                </span>
                <ActionBtn
                  label="Confirm delete"
                  className="btn danger"
                  onRun={async () => {
                    const r = await api.runCleanup([...sel]);
                    setResult(r);
                    setConfirm(false);
                    api.scanCleanup(true).then((env) => setCats(env.data));
                    setSel(new Set());
                  }}
                />
                <button className="btn ghost" onClick={() => setConfirm(false)}>Cancel</button>
              </>
            )}
            <button className="btn ghost" onClick={() => scan(true)}>Re-scan</button>
          </div>
          {result && (
            <div className="mt" style={{ color: "var(--green)" }}>
              ✔ Freed {fmtBytes(result.freedBytes)} · {result.deleted} files deleted · {result.skippedInUse} in-use files skipped
            </div>
          )}
        </Card>
      )}
    </>
  );
}
