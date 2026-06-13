import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner, Badge } from "../components/ui";
import { useLang } from "../i18n";

export default function Startup({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [items, setItems] = useState<any[] | null>(null);
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState<string | null>(null);

  const SCOPE_LABEL: Record<string, string> = {
    hkcu_run: t("scopeHkcuRun"),
    hklm_run: t("scopeHklmRun"),
    hklm_run32: t("scopeHklmRun32"),
    folder_user: t("scopeFolderUser"),
    folder_common: t("scopeFolderCommon"),
  };

  const load = () => api.startupList().then((r) => setItems(r.items));
  useEffect(() => {
    load();
  }, []);

  const toggle = async (it: any) => {
    setBusy(it.scope + it.name);
    setErr("");
    try {
      await api.startupToggle(it.scope, it.name, !it.enabled);
      await load();
    } catch (e: any) {
      setErr(String(e));
    } finally {
      setBusy(null);
    }
  };

  const enabled = (items ?? []).filter((i) => i.enabled).length;

  return (
    <>
      <div className="page-title">{t("startupTitle")}</div>
      <div className="page-sub">{t("startupSub")}</div>

      {!items && <><Spinner /> <span className="muted">{t("startupLoading")}</span></>}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}

      {items && (
        <Card title={`${items.length} ${t("startupEntries")} · ${enabled} ${t("active")}`}>
          <table className="tbl">
            <thead>
              <tr>
                <th>{t("startupProgram")}</th>
                <th>{t("startupCommand")}</th>
                <th>{t("startupSource")}</th>
                <th>{t("status")}</th>
              </tr>
            </thead>
            <tbody>
              {items.map((it, i) => {
                const needsAdmin = it.scope.startsWith("hklm") || it.scope === "folder_common";
                return (
                  <tr key={i} style={{ opacity: it.enabled ? 1 : 0.55 }}>
                    <td style={{ fontWeight: 600 }}>{it.name}</td>
                    <td className="mono muted" style={{ maxWidth: 420, wordBreak: "break-all", fontSize: 11 }}>
                      {it.command}
                    </td>
                    <td className="muted" style={{ whiteSpace: "nowrap" }}>
                      {SCOPE_LABEL[it.scope] ?? it.scope}
                      {needsAdmin && <Badge cls="st-unknown"> admin</Badge>}
                    </td>
                    <td style={{ width: 130 }}>
                      <button
                        className={`btn small ${it.enabled ? "ghost" : ""}`}
                        disabled={busy !== null || (needsAdmin && !admin)}
                        onClick={() => toggle(it)}
                      >
                        {busy === it.scope + it.name ? <Spinner /> : it.enabled ? t("disable") : t("enable")}
                      </button>
                    </td>
                  </tr>
                );
              })}
              {items.length === 0 && (
                <tr><td colSpan={4} className="muted">{t("startupEmpty")}</td></tr>
              )}
            </tbody>
          </table>
          <div className="muted mt" style={{ fontSize: 12 }}>
            {t("startupTip")}
          </div>
        </Card>
      )}
    </>
  );
}
