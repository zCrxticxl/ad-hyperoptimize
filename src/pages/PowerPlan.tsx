import React, { useEffect, useState } from "react";
import { api } from "../api";
import { Card, Spinner } from "../components/ui";
import { HwWarnings, RiskBadge } from "../components/HwWarnings";
import { useHwProfile } from "../hooks/useHwProfile";
import { useLang } from "../i18n";

const ULTIMATE_GUID = "e9a42b02-d5df-448d-aa00-03f14749eb61";

// Map a plan's GUID to a risk-badge id. The 3 non-Ultimate Windows defaults
// have no backend risk entry (intentional — they're always safe, badge
// defaults to green "OK"). Ultimate uses its real laptop-conditional risk.
// Anything else (Winhance, OEM-bundled, hand-made custom plans) is unknown —
// we can't see its Advanced Power Settings, so it's always flagged.
const PLAN_RISK_ID: Record<string, string> = {
  "381b4222-f694-41f0-9685-ff5bb260df2e": "power_plan_balanced",
  "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c": "power_plan_high_perf",
  "a1841308-3541-4fab-bc81-f71556f20b4a": "power_plan_power_saver",
  "e9a42b02-d5df-448d-aa00-03f14749eb61": "power_plan_ultimate",
};
function riskIdForPlan(guid: string): string {
  return PLAN_RISK_ID[guid.toLowerCase()] ?? "power_plan_custom_unknown";
}

export default function PowerPlan({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData] = useState<any>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [log, setLog] = useState("");
  const [newName, setNewName] = useState("");
  const [showCreate, setShowCreate] = useState(false);
  const profile = useHwProfile();
  const ultimateRisk = profile?.tweakRisks?.["power_plan_ultimate"];

  const PLAN_DESC: Record<string, string> = {
    "381b4222-f694-41f0-9685-ff5bb260df2e": t("pwrDescBalanced"),
    "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c": t("pwrDescHighPerf"),
    "a1841308-3541-4fab-bc81-f71556f20b4a": t("pwrDescPowerSaver"),
    "e9a42b02-d5df-448d-aa00-03f14749eb61": t("pwrDescUltimate"),
  };

  const load = async () => setData(await api.powerplanList());
  useEffect(() => { load(); }, []);

  const act = async (label: string, fn: () => Promise<string>) => {
    setBusy(label);
    try { setLog(await fn()); await load(); }
    catch (e: any) { setLog(String(e)); }
    finally { setBusy(null); }
  };

  const plans: any[] = data?.plans ?? [];
  const hasUltimate = data?.ultimateAvailable ?? false;
  const ultimateGuid = data?.ultimateGuid ?? "";

  const activeGuid = plans.find((p: any) => p.active)?.guid ?? "";

  return (
    <>
      <div className="page-title">🔋 {t("pwrTitle")}</div>
      <div className="page-sub">
        {t("pwrSub")}
        {!admin && <span style={{ color: "var(--orange)" }}> · {t("pwrAdminRequiredHint")}</span>}
      </div>
      <HwWarnings page="power_plan" />

      {!hasUltimate && (
        <Card title={`⚡ ${t("pwrUnlockUltimateTitle")}`} style={{ marginBottom: 14 }}>
          <div className="muted" style={{ fontSize: 13, marginBottom: 10 }}>
            {t("pwrUnlockUltimateDesc")}
          </div>
          {ultimateRisk && <div style={{ marginBottom: 8 }}><RiskBadge id="power_plan_ultimate" /></div>}
          <button
            className="btn"
            onClick={() => {
              if (ultimateRisk && !window.confirm(`${ultimateRisk.title}\n\n${ultimateRisk.message}\n\n${t("pwrContinueAnyway")}`)) return;
              act("unlock", api.powerplanUnlockUltimate);
            }}
            disabled={busy !== null || !admin}
          >
            {busy === "unlock" ? <><Spinner /> {t("pwrUnlocking")}</> : `⚡ ${t("pwrUnlockUltimateBtn")}`}
          </button>
          {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 8 }}>{log}</div>}
        </Card>
      )}

      <Card title={`${t("pwrPlansTitle")} (${plans.length})`}>
        {!data ? <Spinner /> : (
          <>
            {plans.map((p: any) => (
              <div
                key={p.guid}
                className="row"
                style={{
                  padding: "12px 0",
                  borderBottom: "1px solid var(--border)",
                  alignItems: "flex-start",
                  gap: 12,
                }}
              >
                <div style={{ flex: 1 }}>
                  <div className="row" style={{ alignItems: "center", gap: 8, marginBottom: 3 }}>
                    {p.active && <span style={{ color: "var(--green)", fontWeight: 700 }}>●</span>}
                    <b style={{ color: p.active ? "var(--accent)" : "var(--fg)" }}>{p.name}</b>
                    {p.active && <span style={{ fontSize: 11, color: "var(--green)" }}>{t("pwrPlanActive")}</span>}
                    <RiskBadge id={riskIdForPlan(p.guid)} />
                  </div>
                  <div className="muted" style={{ fontSize: 12 }}>
                    {PLAN_DESC[p.guid.toLowerCase()] ?? t("pwrDescCustom")}
                  </div>
                  <div className="mono muted" style={{ fontSize: 10, marginTop: 2 }}>{p.guid}</div>
                </div>
                <div className="row" style={{ gap: 6, flexShrink: 0 }}>
                  {!p.active && (
                    <button
                      className="btn small"
                      onClick={() => {
                        if (p.guid.toLowerCase() === ULTIMATE_GUID && ultimateRisk &&
                            !window.confirm(`${ultimateRisk.title}\n\n${ultimateRisk.message}\n\n${t("pwrContinueAnyway")}`)) return;
                        act(p.guid, () => api.powerplanSet(p.guid));
                      }}
                      disabled={busy !== null || !admin}
                    >
                      {busy === p.guid ? "…" : t("pwrSetActive")}
                    </button>
                  )}
                  {!["381b4222-f694-41f0-9685-ff5bb260df2e",
                     "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c",
                     "a1841308-3541-4fab-bc81-f71556f20b4a",
                     "e9a42b02-d5df-448d-aa00-03f14749eb61"].includes(p.guid.toLowerCase()) && (
                    <button
                      className="btn small ghost danger"
                      onClick={async () => {
                        if (!window.confirm(`${t("pwrDeleteConfirm")} "${p.name}"?`)) return;
                        act(`del-${p.guid}`, () => api.powerplanDelete(p.guid));
                      }}
                      disabled={busy !== null || p.active || !admin}
                    >
                      🗑
                    </button>
                  )}
                </div>
              </div>
            ))}

            {log && <div className="mono muted" style={{ fontSize: 11, marginTop: 10 }}>{log}</div>}

            <div style={{ marginTop: 14 }}>
              {!showCreate ? (
                <button className="btn small ghost" onClick={() => setShowCreate(true)} disabled={!admin}>
                  + {t("pwrCreateCustomBtn")}
                </button>
              ) : (
                <div className="row" style={{ gap: 8, alignItems: "center", flexWrap: "wrap" }}>
                  <input
                    placeholder={t("pwrPlanNamePlaceholder")}
                    value={newName}
                    onChange={e => setNewName(e.target.value)}
                    style={{ padding: "4px 8px", fontSize: 13, background: "var(--bg2)", border: "1px solid var(--border)", borderRadius: 4, color: "var(--fg)", width: 200 }}
                  />
                  <span className="muted" style={{ fontSize: 12 }}>{t("pwrBasedOn")}</span>
                  <button
                    className="btn small ghost"
                    onClick={() => act("create", () => api.powerplanCreate(newName, activeGuid))}
                    disabled={!newName.trim() || busy !== null}
                  >
                    {t("pwrDuplicateActive")}
                  </button>
                  <button className="btn small ghost" onClick={() => setShowCreate(false)}>{t("cancel")}</button>
                </div>
              )}
            </div>
          </>
        )}
      </Card>

      <div className="muted" style={{ fontSize: 12, marginTop: 14 }}>
        {t("pwrUltimateTip")}
      </div>
    </>
  );
}
