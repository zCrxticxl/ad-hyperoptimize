import React, { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";

const PRIORITIES = ["Idle", "BelowNormal", "Normal", "AboveNormal", "High", "RealTime"];
const PERM_PRIORITIES = ["Idle", "BelowNormal", "Normal", "AboveNormal", "High"];
const INTERVALS = [
  { v: 0, label: "aus" },
  { v: 1000, label: "1s" },
  { v: 2000, label: "2s" },
  { v: 3000, label: "3s" },
  { v: 5000, label: "5s" },
  { v: 10000, label: "10s" },
];

function parseCores(s: string, max: number): number {
  let mask = 0;
  for (const part of s.split(",").map((x) => x.trim()).filter(Boolean)) {
    const m = part.match(/^(\d+)\s*-\s*(\d+)$/);
    if (m) {
      for (let c = +m[1]; c <= +m[2] && c < max; c++) mask |= 1 << c;
    } else if (/^\d+$/.test(part) && +part < max) {
      mask |= 1 << +part;
    }
  }
  return mask;
}

export default function Processes() {
  const { t } = useLang();
  const [data, setData] = useState<any | null>(null);
  const [perm, setPerm] = useState<Record<string, string>>({});
  const [filter, setFilter] = useState("");
  const [sortKey, setSortKey] = useState<"cpu" | "memMb">("cpu");
  const [interval, setIntervalMs] = useState(0);
  const [msg, setMsg] = useState("");
  const [confirmKill, setConfirmKill] = useState<number | null>(null);
  const [expand, setExpand] = useState<{ pid: number; kind: "affinity" | "perm" } | null>(null);
  const [affinityText, setAffinityText] = useState("");
  const [permPrio, setPermPrio] = useState("High");
  const timer = useRef<number | null>(null);

  const load = () => api.procList().then(setData).catch((e) => setMsg(String(e)));
  const loadPerm = () => api.permPriorityList().then(setPerm).catch(() => {});

  useEffect(() => {
    load();
    loadPerm();
  }, []);

  useEffect(() => {
    if (timer.current) window.clearInterval(timer.current);
    if (interval > 0) timer.current = window.setInterval(load, interval);
    return () => {
      if (timer.current) window.clearInterval(timer.current);
    };
  }, [interval]);

  const procs = (data?.processes ?? [])
    .filter((p: any) => !filter || p.name.toLowerCase().includes(filter.toLowerCase()))
    .sort((a: any, b: any) => b[sortKey] - a[sortKey])
    .slice(0, 80);

  const act = async (fn: () => Promise<any>, ok: string) => {
    setMsg("");
    try {
      await fn();
      setMsg(ok);
      load();
      loadPerm();
    } catch (e: any) {
      setMsg(String(e));
    }
  };

  return (
    <>
      <div className="page-title">{t("procTitle")}</div>
      <div className="page-sub">{t("procSub")}</div>

      <Card title={`${t("procCard")} ${data ? `(${data.processes.length}, ${t("procShown")})` : ""}`}>
        <div className="row" style={{ marginBottom: 10 }}>
          <input
            className="select"
            placeholder="Filter…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            style={{ width: 180 }}
          />
          <select className="select" value={sortKey} onChange={(e) => setSortKey(e.target.value as any)}>
            <option value="cpu">{t("sortCpu")}</option>
            <option value="memMb">{t("sortRam")}</option>
          </select>
          <button className="btn ghost small" onClick={load}>{t("procRefresh")}</button>
          <span className="muted" style={{ fontSize: 12 }}>Auto-Refresh:</span>
          <select
            className="select"
            value={interval}
            onChange={(e) => setIntervalMs(+e.target.value)}
            style={{ padding: "4px 8px" }}
          >
            {INTERVALS.map((i) => <option key={i.v} value={i.v}>{i.label}</option>)}
          </select>
          {msg && <span style={{ color: msg.startsWith("✔") ? "var(--green)" : "var(--yellow)", fontSize: 12 }}>{msg}</span>}
        </div>

        {!data && <Spinner />}
        {data && (
          <table className="tbl">
            <thead>
              <tr>
                <th>{t("procProcess")}</th>
                <th>PID</th>
                <th>CPU %</th>
                <th>RAM</th>
                <th>{t("procPriority")}</th>
                <th></th>
                <th></th>
              </tr>
            </thead>
            <tbody>
              {procs.map((p: any) => {
                const permFor = perm[p.name?.toLowerCase?.() ?? ""];
                return (
                  <React.Fragment key={p.pid}>
                    <tr>
                      <td style={{ fontWeight: 600 }} title={p.path}>
                        {p.name}
                        {p.protected && <span className="muted" style={{ fontSize: 10 }}> 🔒 system</span>}
                        {permFor && (
                          <span style={{ color: "var(--cyan)", fontSize: 10 }}> ★ {t("procPerm")} {permFor}</span>
                        )}
                      </td>
                      <td className="muted">{p.pid}</td>
                      <td style={{ color: p.cpu > 20 ? "var(--orange)" : undefined }}>{p.cpu}</td>
                      <td className="muted">{p.memMb} MB</td>
                      <td>
                        <select
                          className="select"
                          defaultValue=""
                          style={{ padding: "3px 6px", fontSize: 11 }}
                          onChange={(e) => {
                            const v = e.target.value;
                            e.target.value = "";
                            if (!v) return;
                            if (v === "RealTime" && !window.confirm(t("procRealTimeWarn"))) return;
                            act(() => api.procPriority(p.pid, v), `✔ ${p.name} → ${v} (${t("procUntilRestart")})`);
                          }}
                        >
                          <option value="">{t("procNow")}</option>
                          {PRIORITIES.map((pr) => <option key={pr} value={pr}>{pr}</option>)}
                        </select>
                        <button
                          className="btn small ghost"
                          style={{ marginLeft: 4 }}
                          onClick={() => setExpand(expand !== null && expand.pid === p.pid && expand.kind === "perm" ? null : { pid: p.pid, kind: "perm" })}
                        >
                          ★
                        </button>
                      </td>
                      <td>
                        <button
                          className="btn small ghost"
                          onClick={() => {
                            setExpand(expand !== null && expand.pid === p.pid && expand.kind === "affinity" ? null : { pid: p.pid, kind: "affinity" });
                            setAffinityText("");
                          }}
                        >
                          {t("procCores")}…
                        </button>
                      </td>
                      <td style={{ width: 110 }}>
                        {p.protected ? (
                          <span className="muted" style={{ fontSize: 11 }}>{t("procProtected")}</span>
                        ) : confirmKill === p.pid ? (
                          <>
                            <button className="btn small danger" onClick={() => {
                              act(() => api.procKill(p.pid), `✔ ${p.name} ${t("procKill")}`);
                              setConfirmKill(null);
                            }}>{t("confirm")}</button>
                            <button className="btn small ghost" onClick={() => setConfirmKill(null)}>✕</button>
                          </>
                        ) : (
                          <button className="btn small ghost" onClick={() => setConfirmKill(p.pid)}>{t("procKill")}</button>
                        )}
                      </td>
                    </tr>
                    {expand !== null && expand.pid === p.pid && expand.kind === "affinity" && (
                      <tr>
                        <td colSpan={7} style={{ background: "var(--bg2)" }}>
                          <div className="row">
                            <span className="muted" style={{ fontSize: 12 }}>
                              {t("procCores")} (0–{data.coreCount - 1}), {t("procCoresEg")}:
                            </span>
                            <input className="select" value={affinityText} onChange={(e) => setAffinityText(e.target.value)} style={{ width: 160 }} />
                            <button className="btn small" onClick={() => {
                              const mask = parseCores(affinityText, data.coreCount);
                              if (!mask) { setMsg(t("procCoresInvalid")); return; }
                              act(() => api.procAffinity(p.pid, mask), `✔ Affinity set (${affinityText})`);
                              setExpand(null);
                            }}>{t("set")}</button>
                          </div>
                        </td>
                      </tr>
                    )}
                    {expand !== null && expand.pid === p.pid && expand.kind === "perm" && (
                      <tr>
                        <td colSpan={7} style={{ background: "var(--bg2)" }}>
                          <div className="row">
                            <span className="muted" style={{ fontSize: 12 }}>
                              {t("procPermDesc")} <b>{p.name}</b> {t("procPermSuffix")}
                            </span>
                            <select className="select" value={permPrio} onChange={(e) => setPermPrio(e.target.value)} style={{ padding: "4px 8px" }}>
                              {PERM_PRIORITIES.map((pr) => <option key={pr} value={pr}>{pr}</option>)}
                            </select>
                            <button className="btn small" onClick={() => {
                              act(() => api.permPrioritySet(p.name, permPrio), `✔ ${p.name} permanent → ${permPrio}`);
                              setExpand(null);
                            }}>{t("procPermSet")}</button>
                            {permFor && (
                              <button className="btn small ghost" onClick={() => {
                                act(() => api.permPriorityRemove(p.name), `✔ Override removed: ${p.name}`);
                                setExpand(null);
                              }}>{t("procPermRemove")}</button>
                            )}
                          </div>
                        </td>
                      </tr>
                    )}
                  </React.Fragment>
                );
              })}
            </tbody>
          </table>
        )}
        </Card>
    </>
  );
}
