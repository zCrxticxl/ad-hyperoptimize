import React, { useEffect, useState } from "react";
import { api } from "../../api";
import { Card, Spinner } from "../ui";
import { useLang, TKey } from "../../i18n";
import { gpuLoad } from "../../lib/gpuLoad";
import { RamAssistant } from "./RamAssistant";

type Link = { name?: string; curGen: number; maxGen: number; curWidth: number; maxWidth: number; verdict: string; util?: number };
const VCOLOR: Record<string, string> = { ok: "var(--green)", gen: "var(--orange)", lanes: "var(--red)", unknown: "var(--muted)" };
const linkText = (l: Link) => `Gen${l.curGen || "?"} x${l.curWidth || "?"} / max Gen${l.maxGen || "?"} x${l.maxWidth || "?"}`;

/** RAM channels, PCIe links, power-plan limits, USB layout. */
export function HardwareCard({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData] = useState<any>(null);
  const [busy, setBusy] = useState("");
  const [live, setLive] = useState<Link | null | "none">(null);
  const [msg, setMsg] = useState("");

  const load = () => { setData(null); api.hwCheck().then(setData).catch((e) => setData({ error: String(e) })); };
  useEffect(load, []);

  const testLink = async () => {
    setBusy("link"); setLive(null);
    let best: Link | null = null;
    const done = gpuLoad(5000);
    for (let i = 0; i < 10; i++) {
      await new Promise((r) => setTimeout(r, 500));
      const l: Link | null = await api.gpuLinkLive().catch(() => null);
      if (l && (!best || l.curGen * 100 + l.curWidth > best.curGen * 100 + best.curWidth)) best = l;
    }
    await done;
    setLive(best ?? "none");
    setBusy("");
  };

  const fix = async (alias: string) => {
    setBusy(alias); setMsg("");
    try { await api.powerFix(alias); setMsg(t("hwFixed")); } catch (e: any) { setMsg(String(e)); }
    finally { setBusy(""); load(); }
  };
  const undo = async (entry: string) => {
    setBusy(entry); setMsg("");
    try { await api.revertEntry(entry); setMsg(t("hwUndone")); } catch (e: any) { setMsg(String(e)); }
    finally { setBusy(""); load(); }
  };

  const val = (alias: string, v: number) =>
    alias === "PERFBOOSTMODE" ? (v <= 6 ? t(`hwBoost_${v}` as TKey) : String(v)) : `${v} %`;
  const section = (title: string) => <div style={{ fontWeight: 700, fontSize: 13, margin: "14px 0 6px" }}>{title}</div>;
  const hint = (k: string, color = "var(--muted)") => <div key={k} style={{ fontSize: 12, marginTop: 3, color }}>● {t(k as TKey)}</div>;

  return (
    <Card title={t("hwTitle")}>
      <div className="row" style={{ justifyContent: "space-between", gap: 8 }}>
        <div className="muted" style={{ fontSize: 12 }}>{t("hwDesc")}</div>
        <button className="btn small ghost" onClick={load}>↺</button>
      </div>
      {!data && <Spinner />}
      {data?.error && <div style={{ color: "var(--red)", fontSize: 12 }}>{data.error}</div>}
      {data && !data.error && (
        <>
          {data.windows && (
            <>
              {section(t("hwWin"))}
              <div style={{ fontSize: 13 }}>
                Windows {data.windows.version || ""} ({data.windows.build}.{data.windows.ubr}) · {data.windows.cpu}
              </div>
              {data.windows.branchFix !== "notApplicable" && hint(`hwFix_${data.windows.branchFix}`,
                data.windows.branchFix === "included" ? "var(--green)" : "var(--orange)")}
            </>
          )}
          {section(t("hwRam"))}
          <div style={{ fontSize: 13, color: data.ram.mode === "multi" ? "var(--green)" : data.ram.mode === "unknown" ? "var(--muted)" : "var(--red)" }}>
            {t(`hwRamMode_${data.ram.mode}` as TKey).replace("{c}", data.ram.channels.join(", "))}
          </div>
          <div className="muted" style={{ fontSize: 12 }}>
            {data.ram.modules.map((m: any) => `${m.locator || m.bank} · ${m.gb} GB · ${m.speed} MT/s`).join("  |  ")}
            {data.ram.slots ? ` · ${t("hwSlots").replace("{n}", String(data.ram.slots))}` : ""}
          </div>
          {data.ram.hints.map((h: string) => hint(`hwRamHint_${h}`, h === "wrongSlots" || h === "addModule" ? "var(--orange)" : "var(--muted)"))}
          {data.ram.plan && <RamAssistant plan={data.ram.plan} onRecheck={load} />}

          {section(t("hwGpuLink"))}
          {data.links.gpus.map((g: Link, i: number) => (
            <div key={i} style={{ fontSize: 13 }}>
              {g.name}: <span style={{ color: VCOLOR[g.verdict] }}>{linkText(g)}</span>
              {g.verdict === "lanes" && hint("hwLinkLanes", "var(--red)")}
            </div>
          ))}
          <div className="row" style={{ gap: 8, marginTop: 6, alignItems: "center", flexWrap: "wrap" }}>
            <button className="btn small" disabled={!!busy} onClick={testLink}>{busy === "link" ? <><Spinner /> {t("hwTesting")}</> : t("hwTestLoad")}</button>
            {live === "none" && <span className="muted" style={{ fontSize: 12 }}>{t("hwNoLive")}</span>}
            {live && live !== "none" && (
              <span style={{ fontSize: 13, color: VCOLOR[live.verdict] }}>{t("hwUnderLoad")}: {linkText(live)}</span>
            )}
          </div>
          {live && live !== "none" && live.verdict !== "ok" && hint(live.verdict === "lanes" ? "hwLinkLanes" : "hwLinkGen", VCOLOR[live.verdict])}
          <div className="muted" style={{ fontSize: 11, marginTop: 4 }}>{t("hwLinkNote")}</div>

          {section(t("hwNvme"))}
          {data.links.nvme.length === 0 && <div className="muted" style={{ fontSize: 12 }}>{t("hwNoNvme")}</div>}
          {data.links.nvme.map((n: Link, i: number) => (
            <div key={i} style={{ fontSize: 13 }}>
              {n.name}: <span style={{ color: VCOLOR[n.verdict] }}>{linkText(n)}</span>
              {n.verdict !== "ok" && n.verdict !== "unknown" && hint(n.verdict === "lanes" ? "hwNvmeLanes" : "hwNvmeGen", VCOLOR[n.verdict])}
            </div>
          ))}

          {section(t("hwPower"))}
          {data.power.settings.map((s: any) => {
            const issue = s.acIssue || s.dcIssue;
            return (
              <div key={s.alias} className="row" style={{ gap: 8, alignItems: "center", padding: "3px 0", flexWrap: "wrap" }}>
                <span style={{ fontSize: 13, flex: 1 }}>
                  {t(`hwPow_${s.alias}` as TKey)}: <b style={{ color: issue ? "var(--orange)" : "var(--green)" }}>{t("hwAc")} {val(s.alias, s.ac)} · {t("hwDc")} {val(s.alias, s.dc)}</b>
                  {issue && <span className="muted" style={{ fontSize: 12 }}> – {t(`hwPowIssue_${issue}` as TKey)}</span>}
                </span>
                {s.open ? (
                  <button className="btn small ghost" disabled={!!busy || !admin} onClick={() => undo(s.open)}>{t("revert")}</button>
                ) : issue ? (
                  <button className="btn small" disabled={!!busy || !admin} onClick={() => fix(s.alias)}>{busy === s.alias ? <Spinner /> : t("hwReset")}</button>
                ) : null}
              </div>
            );
          })}

          {section(t("hwUsb"))}
          {data.usb.controllers.length === 0 && <div className="muted" style={{ fontSize: 12 }}>{t("hwNoUsb")}</div>}
          {data.usb.controllers.map((c: any) => (
            <div key={c.controller} style={{ padding: "4px 0", borderBottom: "1px solid var(--border)" }}>
              <div style={{ fontSize: 13 }}>{c.name || c.controller}{c.thirdParty && <span className="muted" style={{ fontSize: 11 }}> · {t("hwUsbAddon")}</span>}</div>
              <div className="muted" style={{ fontSize: 12 }}>
                {c.devices.map((d: any) => `${t(`hwUsbKind_${d.kind}` as TKey)}: ${d.name}${d.hub ? ` (${t("hwUsbHub")})` : ""}`).join(" · ")}
              </div>
              {c.hints.map((h: string) => hint(`hwUsbHint_${h}`, "var(--orange)"))}
            </div>
          ))}
          {msg && <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>{msg}</div>}
          {!admin && <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>{t("adminRestart")}</div>}
        </>
      )}
    </Card>
  );
}
