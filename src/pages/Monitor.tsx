import React, { useEffect, useRef, useState } from "react";
import { api, onMetrics, Metrics } from "../api";
import { Card, Bar } from "../components/ui";
import {
  LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid,
} from "recharts";

const MAX_POINTS = 60;

export default function Monitor() {
  const [latest, setLatest] = useState<Metrics | null>(null);
  const [series, setSeries] = useState<any[]>([]);
  const unlisten = useRef<(() => void) | null>(null);

  useEffect(() => {
    api.startMonitor();
    onMetrics((m) => {
      setLatest(m);
      setSeries((s) => {
        const next = [...s, {
          t: m.t,
          cpu: m.cpuTotal,
          mem: (m.memUsedMb / m.memTotalMb) * 100,
          rx: m.netRxKbs,
          tx: m.netTxKbs,
        }];
        return next.length > MAX_POINTS ? next.slice(-MAX_POINTS) : next;
      });
    }).then((u) => (unlisten.current = u));
    return () => {
      unlisten.current?.();
      api.stopMonitor();
    };
  }, []);

  const chart = (key: string, color: string, unit: string, domain?: [number, number]) => (
    <ResponsiveContainer width="100%" height={170}>
      <LineChart data={series}>
        <CartesianGrid stroke="#232b3d" strokeDasharray="3 3" />
        <XAxis dataKey="t" tick={{ fill: "#8b95a8", fontSize: 10 }} interval="preserveStartEnd" />
        <YAxis tick={{ fill: "#8b95a8", fontSize: 10 }} domain={domain ?? [0, "auto"]} width={38} />
        <Tooltip
          contentStyle={{ background: "#161b27", border: "1px solid #232b3d", borderRadius: 8 }}
          formatter={(v: any) => [`${Number(v).toFixed(1)} ${unit}`, key]}
        />
        <Line type="monotone" dataKey={key} stroke={color} dot={false} strokeWidth={2} isAnimationActive={false} />
      </LineChart>
    </ResponsiveContainer>
  );

  return (
    <>
      <div className="page-title">Live Monitor</div>
      <div className="page-sub">1-second sampling · CPU, memory, network, disks, top processes.</div>

      <div className="grid grid-4">
        <Card title="CPU"><div className="stat-big">{latest?.cpuTotal.toFixed(1) ?? "—"}%</div>
          <div className="stat-sub">{latest?.freqMhz ? `${latest.freqMhz} MHz` : ""}</div></Card>
        <Card title="Memory"><div className="stat-big">{latest ? ((latest.memUsedMb / latest.memTotalMb) * 100).toFixed(0) : "—"}%</div>
          <div className="stat-sub">{latest ? `${(latest.memUsedMb / 1024).toFixed(1)} / ${(latest.memTotalMb / 1024).toFixed(1)} GB` : ""}</div></Card>
        <Card title="Net ↓"><div className="stat-big">{latest ? fmtRate(latest.netRxKbs) : "—"}</div></Card>
        <Card title="Net ↑"><div className="stat-big">{latest ? fmtRate(latest.netTxKbs) : "—"}</div></Card>
      </div>

      <div className="grid grid-2 mt">
        <Card title="CPU usage %">{chart("cpu", "#4f8cff", "%", [0, 100])}</Card>
        <Card title="Memory usage %">{chart("mem", "#7c5cff", "%", [0, 100])}</Card>
      </div>
      <div className="grid grid-2 mt">
        <Card title="Network ↓ KB/s">{chart("rx", "#3fd68f", "KB/s")}</Card>
        <Card title="Network ↑ KB/s">{chart("tx", "#ffd166", "KB/s")}</Card>
      </div>

      <div className="grid grid-2 mt">
        <Card title="Top processes — CPU">
          <table className="tbl">
            <tbody>
              {latest?.topCpu.map((p, i) => (
                <tr key={i}><td>{p.name}</td><td style={{ width: 70 }}>{p.cpu}%</td><td className="muted" style={{ width: 90 }}>{p.memMb} MB</td></tr>
              ))}
            </tbody>
          </table>
        </Card>
        <Card title="Top processes — Memory">
          <table className="tbl">
            <tbody>
              {latest?.topMem.map((p, i) => (
                <tr key={i}><td>{p.name}</td><td style={{ width: 90 }}>{p.memMb} MB</td><td className="muted" style={{ width: 70 }}>{p.cpu}%</td></tr>
              ))}
            </tbody>
          </table>
        </Card>
      </div>

      <Card title="Disks" style={{ marginTop: 14 }}>
        {latest?.disks.map((d, i) => {
          const used = ((d.totalGb - d.freeGb) / d.totalGb) * 100;
          return (
            <div key={i} className="mt">
              <div className="row" style={{ justifyContent: "space-between" }}>
                <span>{d.name}</span>
                <span className="muted">{d.freeGb.toFixed(1)} GB free of {d.totalGb.toFixed(0)} GB</span>
              </div>
              <Bar pct={used} color={used > 90 ? "var(--red)" : undefined} />
            </div>
          );
        })}
      </Card>
    </>
  );
}

function fmtRate(kbs: number) {
  return kbs >= 1024 ? (kbs / 1024).toFixed(1) + " MB/s" : kbs.toFixed(0) + " KB/s";
}
