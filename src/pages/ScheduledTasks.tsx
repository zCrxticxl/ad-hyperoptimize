import React, { useEffect, useState, useMemo } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";

type Task = {
  path: string;
  name: string;
  state: string;
  enabled: boolean;
  isBloat: boolean;
  reason: string;
};

type Tab = "bloat" | "all";

function pathCategory(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] || path;
}

export default function ScheduledTasks({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData] = useState<{ tasks: Task[]; bloatCount: number } | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("bloat");
  const [search, setSearch] = useState("");

  const load = () =>
    api
      .schedTasksList()
      .then((r) => setData(r))
      .catch((e) => setErr(String(e)));

  useEffect(() => { load(); }, []);

  const tasks = useMemo(() => {
    if (!data) return [];
    let list = data.tasks;
    if (tab === "bloat") list = list.filter((task) => task.isBloat);
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter(
        (task) => task.name.toLowerCase().includes(q) || task.path.toLowerCase().includes(q)
      );
    }
    return list.sort((a, b) => a.path.localeCompare(b.path) || a.name.localeCompare(b.name));
  }, [data, tab, search]);

  const toggle = async (task: Task) => {
    const key = task.path + task.name;
    setBusy(key);
    setErr("");
    try {
      await api.schedTaskToggle(task.path, task.name, !task.enabled);
      await load();
    } catch (e: any) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const totalCount = data?.tasks.length ?? 0;
  const bloatCount = data?.bloatCount ?? 0;
  const activeCount = tasks.filter((task) => task.enabled).length;

  return (
    <>
      <div className="page-title">{t("schedTitle")}</div>
      <div className="page-sub">{t("schedSub")}</div>

      {!data && <><Spinner /> <span className="muted">{t("schedLoading")}</span></>}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}

      {data && (
        <>
          {/* Summary chips */}
          <div style={{ display: "flex", gap: 12, marginBottom: 16, flexWrap: "wrap" }}>
            <div className="stat-chip">
              <span className="chip-val">{totalCount}</span>
              <span className="chip-lbl">{t("schedTotal")}</span>
            </div>
            <div className="stat-chip" style={{ borderColor: "var(--yellow)" }}>
              <span className="chip-val" style={{ color: "var(--yellow)" }}>{bloatCount}</span>
              <span className="chip-lbl">{t("schedBloat")}</span>
            </div>
            <div className="stat-chip" style={{ borderColor: "var(--green)" }}>
              <span className="chip-val" style={{ color: "var(--green)" }}>{activeCount}</span>
              <span className="chip-lbl">{t("schedActive")}</span>
            </div>
          </div>

          {/* Tabs + Search */}
          <div style={{ display: "flex", gap: 8, marginBottom: 14, alignItems: "center" }}>
            <button
              className={`btn small ${tab === "bloat" ? "" : "ghost"}`}
              onClick={() => setTab("bloat")}
            >
              {t("schedBloatTab")} ({bloatCount})
            </button>
            <button
              className={`btn small ${tab === "all" ? "" : "ghost"}`}
              onClick={() => setTab("all")}
            >
              {t("schedAllTab")} ({totalCount})
            </button>
            <input
              type="text"
              placeholder={t("search")}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              style={{
                marginLeft: "auto",
                background: "var(--bg2)",
                border: "1px solid var(--border)",
                borderRadius: 6,
                padding: "4px 10px",
                color: "var(--fg)",
                fontSize: 13,
                width: 220,
              }}
            />
          </div>

          <Card title={`${tasks.length} ${t("schedShown")} · ${activeCount} ${t("active")}`}>
            <table className="tbl">
              <thead>
                <tr>
                  <th>{t("schedTaskCol")}</th>
                  <th>{t("schedCategory")}</th>
                  {tab === "bloat" && <th>{t("schedDescription")}</th>}
                  <th>{t("status")}</th>
                  <th></th>
                </tr>
              </thead>
              <tbody>
                {tasks.map((task) => {
                  const key = task.path + task.name;
                  const isBusy = busy === key;
                  return (
                    <tr key={key} style={{ opacity: task.enabled ? 1 : 0.55 }}>
                      <td style={{ fontWeight: 600, minWidth: 220 }}>
                        {task.name}
                        {task.isBloat && tab === "all" && (
                          <span
                            style={{
                              marginLeft: 6,
                              fontSize: 10,
                              background: "var(--yellow)",
                              color: "#000",
                              borderRadius: 4,
                              padding: "1px 5px",
                            }}
                          >
                            bloat
                          </span>
                        )}
                      </td>
                      <td className="muted" style={{ whiteSpace: "nowrap", fontSize: 12 }}>
                        {pathCategory(task.path)}
                      </td>
                      {tab === "bloat" && (
                        <td
                          className="muted"
                          style={{ fontSize: 12, maxWidth: 380, lineHeight: 1.4 }}
                        >
                          {task.reason}
                        </td>
                      )}
                      <td style={{ whiteSpace: "nowrap" }}>
                        <span
                          style={{
                            fontSize: 12,
                            color: task.enabled ? "var(--green)" : "var(--muted)",
                          }}
                        >
                          {task.enabled ? t("schedActiveStatus") : t("schedDisabledStatus")}
                        </span>
                      </td>
                      <td style={{ width: 130 }}>
                        <button
                          className={`btn small ${task.enabled ? "ghost" : ""}`}
                          disabled={isBusy || !admin}
                          title={!admin ? t("schedAdminTitle") : ""}
                          onClick={() => toggle(task)}
                        >
                          {isBusy ? <Spinner /> : task.enabled ? t("disable") : t("enable")}
                        </button>
                      </td>
                    </tr>
                  );
                })}
                {tasks.length === 0 && (
                  <tr>
                    <td colSpan={tab === "bloat" ? 5 : 4} className="muted">
                      {t("schedEmpty")}
                    </td>
                  </tr>
                )}
              </tbody>
            </table>
            {!admin && (
              <div className="muted mt" style={{ fontSize: 12, color: "var(--yellow)" }}>
                {t("schedAdminWarn")}
              </div>
            )}
            <div className="muted mt" style={{ fontSize: 12 }}>
              {t("schedTip")}
            </div>
          </Card>
        </>
      )}
    </>
  );
}
