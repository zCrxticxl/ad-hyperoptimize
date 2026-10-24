import React, { useEffect, useState } from "react";
import { api } from "../../api";
import { Card, Spinner } from "../ui";
import { useLang } from "../../i18n";

type Dev = { name: string; type: string; regPath: string; policy: number | null; cores: number[] | null; openEntry: string | null };

/** Interrupt affinity per GPU / network / USB controller (expert). */
export function IrqCard({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData] = useState<{ cpus: number; devices: Dev[] } | null>(null);
  const [pick, setPick] = useState<Record<string, number>>({});
  const [busy, setBusy] = useState("");
  const [msg, setMsg] = useState("");

  const load = () => { setData(null); api.irqList().then(setData).catch((e) => setMsg(String(e))); };
  useEffect(load, []);

  const apply = async (d: Dev, core: number | null) => {
    if (core != null && !window.confirm(t("irqConfirm").replace("{d}", d.name).replace("{c}", String(core)))) return;
    setBusy(d.regPath); setMsg("");
    try { setMsg(await api.irqSet(d.regPath, core)); } catch (e: any) { setMsg(String(e)); }
    finally { setBusy(""); load(); }
  };

  const cpus = data?.cpus ?? 0;
  return (
    <Card title={t("irqTitle")}>
      <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>{t("irqDesc")}</div>
      {!data && !msg && <Spinner />}
      {data?.devices.map((d) => {
        const current = d.policy === 4 && d.cores?.length ? `CPU ${d.cores.join(", ")}` : t("irqDefault");
        const choice = pick[d.regPath] ?? (cpus > 2 ? 2 : 0);
        return (
          <div key={d.regPath} className="row" style={{ gap: 8, alignItems: "center", padding: "6px 0", borderBottom: "1px solid var(--border)", flexWrap: "wrap" }}>
            <span className="muted" style={{ fontSize: 11, width: 36 }}>[{d.type}]</span>
            <span style={{ flex: 1, fontSize: 13 }}>{d.name}<span className="muted" style={{ fontSize: 12 }}> · {current}</span></span>
            {d.openEntry ? (
              <button className="btn small ghost" disabled={!!busy || !admin} onClick={() => apply(d, null)}>{busy === d.regPath ? <Spinner /> : t("revert")}</button>
            ) : (
              <>
                <select className="input" style={{ width: 90 }} value={choice} onChange={(e) => setPick({ ...pick, [d.regPath]: Number(e.target.value) })}>
                  {Array.from({ length: cpus }, (_, i) => <option key={i} value={i}>CPU {i}</option>)}
                </select>
                <button className="btn small" disabled={!!busy || !admin || cpus === 0 || cpus > 64} onClick={() => apply(d, choice)}>
                  {busy === d.regPath ? <Spinner /> : t("irqPin")}
                </button>
              </>
            )}
          </div>
        );
      })}
      <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>{t("irqHowTo")}</div>
      {msg && <div className="muted" style={{ fontSize: 12, marginTop: 6 }}>{msg}</div>}
    </Card>
  );
}
