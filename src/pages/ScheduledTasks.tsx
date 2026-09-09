import React, { useEffect, useState, useMemo } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";
import { useFeatureFocus } from "../hooks/useFeatureFocus";

type Task = {
  path: string;
  name: string;
  state: string;
  enabled: boolean;
  isBloat: boolean;
  reason: string;
  origin: "windows" | "thirdparty";
  author?: string;
  created?: string | null;
  lastRun?: string | null;
  exec?: string;
};

type Tab = "bloat" | "third" | "all";
type SortKey = "name" | "path" | "created" | "lastrun";

function pathCategory(path: string): string {
  const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
  return parts[parts.length - 1] || path;
}

const fmtDate = (iso: string) =>
  new Date(iso).toLocaleDateString(undefined, { year: "numeric", month: "2-digit", day: "2-digit" });

const fmtDateTime = (iso: string) =>
  new Date(iso).toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });

export default function ScheduledTasks({ admin, focusId }: { admin: boolean; focusId?: string }) {
  const { t } = useLang();
  const [data, setData] = useState<{ tasks: Task[]; bloatCount: number } | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("bloat");
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<{ key: SortKey; dir: 1 | -1 }>({ key: "path", dir: 1 });
  useFeatureFocus(focusId, !!data);

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
    if (tab === "third") list = list.filter((task) => task.origin === "thirdparty");
    if (search.trim()) {
      const q = search.toLowerCase();
      list = list.filter(
        (task) =>
          task.name.toLowerCase().includes(q) ||
          task.path.toLowerCase().includes(q) ||
          (task.exec ?? "").toLowerCase().includes(q) ||
          (task.author ?? "").toLowerCase().includes(q)
      );
    }
    const sortVal = (task: Task): string | null => {
      switch (sort.key) {
        case "name": return task.name.toLowerCase();
        case "path": return task.path.toLowerCase();
        case "created": return task.created ?? null;
        case "lastrun": return task.lastRun ?? null;
      }
    };
    return [...list].sort((a, b) => {
      const va = sortVal(a);
      const vb = sortVal(b);
      if (va === null && vb === null) return a.path.localeCompare(b.path) || a.name.localeCompare(b.name);
      if (va === null) return 1;
      if (vb === null) return -1;
      return va.localeCompare(vb) * sort.dir;
    });
  }, [data, tab, search, sort]);

  const toggleSort = (key: SortKey) =>
    setSort((s) => (s.key === key ? { key, dir: s.dir === 1 ? -1 : 1 } : { key, dir: 1 }));

  const th = (key: SortKey, label: string) => {
    const active = sort.key === key;
    return (
      <th
        scope="col"
        onClick={() => toggleSort(key)}
        aria-sort={active ? (sort.dir === 1 ? "ascending" : "descending") : "none"}
        style={{ cursor: "pointer", userSelect: "none", whiteSpace: "nowrap" }}
      >
        {label}
        {active && <span aria-hidden="true">{sort.dir === 1 ? " ▲" : " ▼"}</span>}
      </th>
    );
  };

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
  const thirdCount = data?.tasks.filter((task) => task.origin === "thirdparty").length ?? 0;
  const activeCount = tasks.filter((task) => task.enabled).length;

  return (
    <>
      <h1 className="page-title">{t("schedTitle")}</h1>
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
            <div className="stat-chip" style={{ borderColor: "var(--accent)" }}>
              <span className="chip-val" style={{ color: "var(--accent)" }}>{thirdCount}</span>
              <span className="chip-lbl">{t("schedThird")}</span>
            </div>
            <div className="stat-chip" style={{ borderColor: "var(--green)" }}>
              <span className="chip-val" style={{ color: "var(--green)" }}>{activeCount}</span>
              <span className="chip-lbl">{t("schedActive")}</span>
            </div>
          </div>

          {/* Tabs + Search */}
          <div style={{ display: "flex", gap: 8, marginBottom: 14, alignItems: "center", flexWrap: "wrap" }}>
            <button
              className={`btn small ${tab === "bloat" ? "" : "ghost"}`}
              onClick={() => setTab("bloat")}
            >
              {t("schedBloatTab")} ({bloatCount})
            </button>
            <button
              className={`btn small ${tab === "third" ? "" : "ghost"}`}
              onClick={() => setTab("third")}
            >
              {t("schedThird")} ({thirdCount})
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
                  {th("name", t("schedTaskCol"))}
                  {th("path", t("schedCategory"))}
                  {tab === "bloat" && <th scope="col">{t("schedDescription")}</th>}
                  {th("created", t("schedCreated"))}
                  {th("lastrun", t("schedLastRun"))}
                  <th scope="col">{t("status")}</th>
                  <th scope="col"></th>
                </tr>
              </thead>
              <tbody>
                {tasks.map((task) => {
                  const key = task.path + task.name;
                  const isBusy = busy === key;
                  const isThird = task.origin === "thirdparty";
                  const badgeTip = [task.author, task.exec].filter(Boolean).join("\n");
                  return (
                    <tr key={key} data-focus-id={task.name} style={{ opacity: task.enabled ? 1 : 0.55 }}>
                      <td style={{ fontWeight: 600, minWidth: 220 }}>
                        {task.name}
                        {tab !== "third" && isThird && (
                          <span
                            title={badgeTip}
                            style={{
                              marginLeft: 6,
                              fontSize: 10,
                              background: "var(--accent)",
                              color: "#fff",
                              borderRadius: 4,
                              padding: "1px 5px",
                            }}
                          >
                            {t("schedThird")}
                          </span>
                        )}
                        {task.isBloat && tab !== "bloat" && (
                          <span
                            title={task.reason}
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
                        {tab === "third" && task.exec && (
                          <div
                            className="muted"
                            title={badgeTip}
                            style={{
                              fontWeight: 400,
                              fontSize: 11,
                              maxWidth: 300,
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              whiteSpace: "nowrap",
                            }}
                          >
                            {task.exec}
                          </div>
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
                      <td className="muted" style={{ whiteSpace: "nowrap", fontSize: 12 }} title={task.created ?? ""}>
                        {task.created ? fmtDate(task.created) : "—"}
                      </td>
                      <td className="muted" style={{ whiteSpace: "nowrap", fontSize: 12 }} title={task.lastRun ?? ""}>
                        {task.lastRun ? fmtDateTime(task.lastRun) : "—"}
                      </td>
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
                          disabled={!!busy || isBusy || !admin}
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
                    <td colSpan={tab === "bloat" ? 7 : 6} className="muted">
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
