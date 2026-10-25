import React, { useEffect, useState } from "react";
import { api } from "../../api";
import { Card, Spinner, Switch } from "../ui";
import { useLang } from "../../i18n";

type Game = { exe: string; profile: boolean; forced?: boolean; byApp?: boolean; note?: string };

/** Resizable BAR per game through the NVIDIA driver profile. */
export function RebarCard({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData] = useState<{ systemRebar: boolean | null; bar1Mib?: number; games?: Game[]; error?: string } | null>(null);
  const [extra, setExtra] = useState("");
  const [busy, setBusy] = useState("");
  const [msg, setMsg] = useState("");

  const load = (ex?: string) => { setData(null); api.rebarStatus(ex).then(setData).catch((e) => setData({ systemRebar: null, error: String(e) })); };
  useEffect(() => load(), []);

  const toggle = async (g: Game, on: boolean) => {
    if (on && !window.confirm(t("rebarConfirm").replace("{e}", g.exe))) return;
    setBusy(g.exe); setMsg("");
    try { await api.rebarSet(g.exe, on); setMsg(on ? t("rebarOn") : t("rebarOff")); } catch (e: any) { setMsg(String(e)); }
    finally { setBusy(""); load(extra || undefined); }
  };

  return (
    <Card title={t("rebarTitle")}>
      <div className="muted" style={{ fontSize: 12, marginBottom: 8 }}>{t("rebarDesc")}</div>
      {!data && <Spinner />}
      {data?.error && <div style={{ color: "var(--orange)", fontSize: 12 }}>{data.error}</div>}
      {data && data.systemRebar != null && (
        <div style={{ fontSize: 13, marginBottom: 8, color: data.systemRebar ? "var(--green)" : "var(--red)" }}>
          {data.systemRebar ? t("rebarSystemOn").replace("{m}", String(data.bar1Mib ?? "")) : t("rebarSystemOff")}
        </div>
      )}
      {data?.games?.map((g) => (
        <div key={g.exe} className="row" style={{ gap: 10, alignItems: "center", padding: "6px 0", borderBottom: "1px solid var(--border)" }}>
          <span style={{ flex: 1, fontSize: 13 }}>
            {g.exe}
            {!g.profile && <span className="muted" style={{ fontSize: 12 }}> · {t("rebarNoProfile")}</span>}
            {g.profile && g.forced && !g.byApp && <span className="muted" style={{ fontSize: 12 }}> · {t("rebarAlready")}</span>}
          </span>
          {g.profile && (
            <Switch
              on={!!g.forced}
              label={g.exe}
              busy={busy === g.exe}
              disabled={!admin || data.systemRebar === false || (!!g.forced && !g.byApp)}
              onChange={(next) => toggle(g, next)}
            />
          )}
        </div>
      ))}
      {data && !data.error && (
        <div className="row" style={{ gap: 8, marginTop: 8 }}>
          <input className="input" value={extra} onChange={(e) => setExtra(e.target.value)} placeholder={t("rebarOtherExe")} style={{ flex: 1, maxWidth: 280 }} />
          <button className="btn small ghost" disabled={!extra.trim().toLowerCase().endsWith(".exe")} onClick={() => load(extra.trim())}>{t("rebarAdd")}</button>
        </div>
      )}
      <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>{t("rebarHowTo")}</div>
      {!admin && <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>{t("adminRestart")}</div>}
      {msg && <div className="muted" style={{ fontSize: 12, marginTop: 6 }}>{msg}</div>}
    </Card>
  );
}
