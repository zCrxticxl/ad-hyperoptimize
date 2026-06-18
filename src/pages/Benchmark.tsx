import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, ActionBtn } from "../components/ui";
import { useLang } from "../i18n";

export default function Benchmark() {
  const { t } = useLang();

  const KINDS = [
    { id: "cpu", name: t("benchKindCpuName"), desc: t("benchKindCpuDesc") },
    { id: "memory", name: t("benchKindMemoryName"), desc: t("benchKindMemoryDesc") },
    { id: "disk", name: t("benchKindDiskName"), desc: t("benchKindDiskDesc") },
  ];

  const [results, setResults] = useState<Record<string, any>>({});
  const [history, setHistory] = useState<any[]>([]);

  const loadHistory = () => api.benchHistory().then(setHistory);
  useEffect(() => { loadHistory(); }, []);

  const fmt = (r: any) => {
    if (!r) return null;
    switch (r.kind) {
      case "cpu":
        return <>{t("benchSingle")}: <b>{r.singleMBs} MB/s</b> · {t("benchMulti")} ({r.threads}t): <b>{r.multiMBs} MB/s</b> · {t("benchScaling")} ×{r.scaling}</>;
      case "memory":
        return <>{t("benchCopyBandwidth")}: <b>{r.copyGBs} GB/s</b></>;
      case "disk":
        return <>{t("benchWrite")}: <b>{r.seqWriteMBs} MB/s</b> · {t("benchRead")}: <b>{r.seqReadMBs} MB/s</b> <span className="muted">({r.note})</span></>;
    }
  };

  return (
    <>
      <div className="page-title">{t("benchTitle")}</div>
      <div className="page-sub">
        {t("benchSub")}
      </div>

      {KINDS.map((k) => (
        <Card title={k.name} key={k.id} style={{ marginBottom: 14 }}>
          <div className="muted" style={{ marginBottom: 10 }}>{k.desc}</div>
          <div className="row">
            <ActionBtn
              label={t("benchRunButton")}
              onRun={async () => {
                const r = await api.runBenchmark(k.id);
                setResults((x) => ({ ...x, [k.id]: r }));
                loadHistory();
              }}
            />
            <span>{fmt(results[k.id]) ?? <span className="muted">{t("benchNotRunYet")}</span>}</span>
          </div>
        </Card>
      ))}

      <Card title={`${t("benchHistoryTitle")} (${history.length} ${t("benchRuns")}) — ${t("benchBeforeAfter")}`}>
        <table className="tbl">
          <thead><tr><th>{t("benchColTime")}</th><th>{t("benchColTest")}</th><th>{t("benchColResult")}</th></tr></thead>
          <tbody>
            {[...history].reverse().slice(0, 25).map((h, i) => (
              <tr key={i}>
                <td className="muted">{h.time ? new Date(h.time).toLocaleString() : "—"}</td>
                <td>{h.kind}</td>
                <td>{fmt(h)}</td>
              </tr>
            ))}
            {history.length === 0 && <tr><td colSpan={3} className="muted">{t("benchNoRuns")}</td></tr>}
          </tbody>
        </table>
      </Card>
    </>
  );
}
