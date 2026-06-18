import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { useLang } from "../i18n";

export default function CtxMenuCleaner({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData]     = useState<any>(null);
  const [busy, setBusy]     = useState<string | null>(null);
  const [log, setLog]       = useState("");
  const [bulkBusy, setBulkBusy] = useState(false);

  const load = async () => setData(await api.ctxmenuList());
  useEffect(() => { load(); }, []);

  const toggle = async (entry: any, enable: boolean) => {
    setBusy(entry.path);
    try {
      setLog(await api.ctxmenuToggle(entry.path, enable));
      await load();
    } catch (e: any) { setLog(String(e)); }
    finally { setBusy(null); }
  };

  const disableAll = async () => {
    if (!window.confirm(t("ctxmenuDisableAllConfirm"))) return;
    setBulkBusy(true);
    try { setLog(await api.ctxmenuDisableAll()); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBulkBusy(false); }
  };

  const enableAll = async () => {
    setBulkBusy(true);
    try { setLog(await api.ctxmenuEnableAll()); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBulkBusy(false); }
  };

  const entries: any[] = data?.entries ?? [];
  const present  = entries.filter(e => e.present);
  const disabled = present.filter(e => !e.enabled).length;
  const enabled  = present.filter(e =>  e.enabled).length;

  return (
    <>
      <div className="page-title">☰ {t("ctxmenuTitle")}</div>
      <div className="page-sub">
        {t("ctxmenuSub")}
        {!admin && <span style={{ color: "var(--orange)" }}> · {t("ctxmenuAdminHint")}</span>}
      </div>

      <Card title={`${t("ctxmenuEntries")} · ${enabled} ${t("ctxmenuActive")} · ${disabled} ${t("ctxmenuDisabled")}`}>
        {!data ? <Spinner /> : (
          <>
            <div className="row" style={{ gap: 8, marginBottom: 14 }}>
              <button className="btn small danger" onClick={disableAll} disabled={bulkBusy || !admin}>
                {bulkBusy ? "…" : `⛔ ${t("ctxmenuDisableAllBloat")}`}
              </button>
              <button className="btn small ghost" onClick={enableAll} disabled={bulkBusy}>
                ↩ {t("ctxmenuReenableAll")}
              </button>
              <button className="btn small ghost" onClick={load} disabled={bulkBusy}>↺</button>
            </div>

            {entries.map((e: any) => (
              <div
                key={e.path}
                className="row"
                style={{
                  padding: "8px 0",
                  borderBottom: "1px solid var(--border)",
                  alignItems: "flex-start",
                  gap: 12,
                  opacity: !e.present ? 0.4 : 1,
                }}
              >
                <div style={{ flex: 1 }}>
                  <div className="row" style={{ alignItems: "center", gap: 8, marginBottom: 2 }}>
                    <span style={{
                      color: e.enabled ? "var(--green)" : "var(--red)",
                      fontWeight: 700,
                      fontSize: 12,
                    }}>
                      {e.enabled ? `● ${t("ctxmenuOn")}` : `● ${t("ctxmenuOff")}`}
                    </span>
                    <b style={{ fontSize: 13 }}>{e.name}</b>
                    {e.admin && <span className="muted" style={{ fontSize: 10 }}>[{t("ctxmenuAdminTag")}]</span>}
                    {!e.present && <span className="muted" style={{ fontSize: 10 }}>[{t("ctxmenuNotInstalledTag")}]</span>}
                  </div>
                  <div className="muted" style={{ fontSize: 12 }}>{e.desc}</div>
                </div>

                {e.present && (
                  <button
                    className={`btn small ${e.enabled ? "ghost" : ""}`}
                    style={{ flexShrink: 0 }}
                    disabled={busy === e.path || (e.admin && !admin)}
                    onClick={() => toggle(e, !e.enabled)}
                  >
                    {busy === e.path ? "…" : e.enabled ? t("disable") : t("enable")}
                  </button>
                )}
              </div>
            ))}

            {log && (
              <div className="mono muted" style={{ fontSize: 11, marginTop: 10 }}>{log}</div>
            )}

            <div className="muted" style={{ fontSize: 12, marginTop: 14 }}>
              {t("ctxmenuFooterRestart")}
              {" "}{t("ctxmenuFooterNotInstalled")}
            </div>
          </>
        )}
      </Card>
    </>
  );
}
