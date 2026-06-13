import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Badge, Spinner, ActionBtn } from "../components/ui";
import { useLang } from "../i18n";

function delta(before?: number, after?: number) {
  if (!before || !after || before <= 0) return null;
  const pct = ((after - before) / before) * 100;
  const color = Math.abs(pct) < 3 ? "var(--muted)" : pct > 0 ? "var(--green)" : "var(--red)";
  return (
    <span style={{ color, fontWeight: 600 }}>
      {pct > 0 ? "+" : ""}{pct.toFixed(1)}%
    </span>
  );
}

export default function Profiles() {
  const { t } = useLang();
  const [profiles, setProfiles] = useState<any[] | null>(null);
  const [result, setResult] = useState<any | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);

  const load = () => api.profileList().then(setProfiles);
  useEffect(() => {
    load();
  }, []);

  const run = async (id: string, withBench: boolean) => {
    setResult({ running: true, withBench });
    try {
      setResult(await api.profileApply(id, withBench));
    } catch (e: any) {
      setResult({ error: String(e) });
    }
    setConfirm(null);
    load();
  };

  const b = result?.benchBefore;
  const a = result?.benchAfter;

  return (
    <>
      <div className="page-title">{t("profilesTitle")}</div>
      <div className="page-sub">{t("profilesSub")}</div>

      {!profiles && <Spinner />}

      <div className="grid grid-3">
        {(profiles ?? []).map((p) => (
          <Card title={p.name} key={p.id}>
            <div className="muted" style={{ fontSize: 12, lineHeight: 1.5, minHeight: 70 }}>{p.desc}</div>
            <div className="mt">
              {p.tweaks.map((tw: any, i: number) => (
                <div key={i} style={{ fontSize: 12, padding: "2px 0" }}>
                  <Badge cls={`st-${tw.status}`}>{tw.status === "applied" ? "✓" : "○"}</Badge>{" "}
                  {tw.name}
                </div>
              ))}
            </div>
            <div className="row mt">
              {confirm === p.id ? (
                <>
                  <button className="btn small danger" onClick={() => run(p.id, true)}>
                    {t("profilesWithBench")}
                  </button>
                  <button className="btn small" onClick={() => run(p.id, false)}>
                    {t("profilesJustApply")}
                  </button>
                  <button className="btn small ghost" onClick={() => setConfirm(null)}>✕</button>
                </>
              ) : (
                <button
                  className="btn small"
                  disabled={result?.running}
                  onClick={() => setConfirm(p.id)}
                >
                  {t("profilesApply")}
                </button>
              )}
              {p.appliedCount > 0 && (
                <ActionBtn
                  label="Revert"
                  className="btn small ghost"
                  onRun={async () => {
                    setResult(await api.profileRevert(p.id));
                    load();
                  }}
                />
              )}
            </div>
          </Card>
        ))}
      </div>

      {result?.running && (
        <Card title={t("profilesRunning")} style={{ marginTop: 14 }}>
          <Spinner />{" "}
          <span className="muted">
            {result.withBench ? t("profilesRunningBench") : t("profilesRunningApply")}
          </span>
        </Card>
      )}

      {result && !result.running && !result.error && (
        <Card title={t("profilesResult")} style={{ marginTop: 14 }}>
          {result.restorePoint && <div className="muted">🛟 {result.restorePoint}</div>}
          <div className="mt">
            {result.applied && <span style={{ color: "var(--green)" }}>✔ {result.applied.length} {t("profilesApplied")} </span>}
            {result.skipped?.length > 0 && <span className="muted">· {result.skipped.length} {t("profilesAlreadyActive")} </span>}
            {result.reverted && <span style={{ color: "var(--green)" }}>✔ {result.reverted.length} {t("profilesReverted")} </span>}
            {result.failed?.length > 0 && (
              <div style={{ color: "var(--red)" }} className="mt">
                {result.failed.map((f: any, i: number) => (
                  <div key={i}>✘ {f.id}: {f.error}</div>
                ))}
              </div>
            )}
          </div>
          {b && a && (
            <table className="tbl mt">
              <thead><tr><th>Benchmark</th><th>{t("profilesBefore")}</th><th>{t("profilesAfter")}</th><th>Δ</th></tr></thead>
              <tbody>
                <tr>
                  <td>CPU single (MB/s)</td><td>{b.cpu.singleMBs}</td><td>{a.cpu.singleMBs}</td>
                  <td>{delta(b.cpu.singleMBs, a.cpu.singleMBs)}</td>
                </tr>
                <tr>
                  <td>CPU multi (MB/s)</td><td>{b.cpu.multiMBs}</td><td>{a.cpu.multiMBs}</td>
                  <td>{delta(b.cpu.multiMBs, a.cpu.multiMBs)}</td>
                </tr>
                <tr>
                  <td>RAM copy (GB/s)</td><td>{b.memory.copyGBs}</td><td>{a.memory.copyGBs}</td>
                  <td>{delta(b.memory.copyGBs, a.memory.copyGBs)}</td>
                </tr>
                <tr>
                  <td>Disk write (MB/s)</td><td>{b.disk.seqWriteMBs}</td><td>{a.disk.seqWriteMBs}</td>
                  <td>{delta(b.disk.seqWriteMBs, a.disk.seqWriteMBs)}</td>
                </tr>
                <tr>
                  <td>Disk read (MB/s)</td><td>{b.disk.seqReadMBs}</td><td>{a.disk.seqReadMBs}</td>
                  <td>{delta(b.disk.seqReadMBs, a.disk.seqReadMBs)}</td>
                </tr>
              </tbody>
            </table>
          )}
          {result.benchNote && b && <div className="muted mt" style={{ fontSize: 12 }}>{result.benchNote}</div>}
        </Card>
      )}
      {result?.error && (
        <Card title={t("profilesError")} style={{ marginTop: 14 }}>
          <div style={{ color: "var(--red)" }}>{result.error}</div>
        </Card>
      )}
    </>
  );
}
