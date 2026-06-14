import React, { useEffect, useState, useMemo } from "react";
import { api } from "../api";
import { Card, Spinner, Badge } from "../components/ui";
import { useLang } from "../i18n";

type GpuTweak = {
  id: string;
  name: string;
  category: string;
  vendor: string;
  description: string;
  impact: string;
  risk: string;
  reboot: boolean;
  status: "applied" | "not_applied" | "partial" | "unknown";
  needsDriverKey: boolean;
  driverKeyMissing: boolean;
};

type ScanData = {
  vendor: "nvidia" | "amd" | "intel" | "unknown";
  name: string;
  driverKey: string;
  tweaks: GpuTweak[];
  supported: boolean;
};

const VENDOR_COLOR: Record<string, string> = {
  nvidia: "#76b900",
  amd:    "#ed1c24",
  intel:  "#0071c5",
  unknown:"var(--muted)",
};

const STATUS_CLS: Record<string, string> = {
  applied:     "st-applied",
  not_applied: "st-unknown",
  partial:     "st-partial",
  unknown:     "st-unknown",
};

export default function GpuTweaks({ admin }: { admin: boolean }) {
  const { t } = useLang();
  const [data, setData]   = useState<ScanData | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy]   = useState<string | null>(null);
  const [log, setLog]     = useState<string[]>([]);
  const [open, setOpen]   = useState<string | null>(null);
  const [err, setErr]     = useState("");

  const VENDOR_LABEL: Record<string, string> = {
    nvidia: "NVIDIA",
    amd:    "AMD",
    intel:  "Intel",
    unknown: t("unknown"),
  };

  const STATUS_LABEL: Record<string, string> = {
    applied:     t("gpuStatusApplied"),
    not_applied: t("gpuStatusNot"),
    partial:     t("gpuStatusPartial"),
    unknown:     t("unknown"),
  };

  const push = (m: string) =>
    setLog((l) => [`[${new Date().toLocaleTimeString()}] ${m}`, ...l.slice(0, 49)]);

  const refresh = () => {
    setLoading(true);
    setErr("");
    api.gpuScan()
      .then(setData)
      .catch((e) => setErr(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => { refresh(); }, []);

  const doApply = async (tw: GpuTweak) => {
    if (!data) return;
    setBusy(tw.id);
    push(`Applying: ${tw.name}…`);
    try {
      await api.gpuTweakApply(tw.id, data.driverKey);
      push(`✔ ${tw.name} ${t("gpuLogApplied")}${tw.reboot ? ` ${t("gpuRebootLog")}` : ""}`);
    } catch (e: any) {
      push(`✘ ${tw.name}: ${e}`);
    } finally {
      setBusy(null);
      refresh();
    }
  };

  const doRevert = async (tw: GpuTweak) => {
    if (!data) return;
    setBusy(tw.id);
    push(`Reverting: ${tw.name}…`);
    try {
      await api.gpuTweakRevert(tw.id, data.driverKey);
      push(`↩ ${tw.name} ${t("gpuLogReset")}`);
    } catch (e: any) {
      push(`✘ ${tw.name}: ${e}`);
    } finally {
      setBusy(null);
      refresh();
    }
  };

  const applicableTweaks = useMemo(() => {
    if (!data) return [];
    return data.tweaks.filter((tw: any) => tw.applicable !== false);
  }, [data]);

  const cats = useMemo(() => {
    return [...new Set(applicableTweaks.map((tw: any) => tw.category))];
  }, [applicableTweaks]);

  const appliedCount = applicableTweaks.filter((tw: any) => tw.status === "applied").length;
  const totalCount   = applicableTweaks.length;

  return (
    <>
      <div className="page-title">GPU Tweaks</div>
      <div className="page-sub">{t("gpuSub")}</div>

      {loading && <><Spinner /> <span className="muted">{t("gpuLoading")}</span></>}
      {err && <div style={{ color: "var(--red)", marginBottom: 10 }}>{err}</div>}

      {data && (
        <>
          {/* GPU info card */}
          <Card title={t("gpuDetected")}>
            <div style={{ display: "flex", alignItems: "center", gap: 20, flexWrap: "wrap" }}>
              <div
                style={{
                  background: `${VENDOR_COLOR[data.vendor]}22`,
                  border: `1px solid ${VENDOR_COLOR[data.vendor]}66`,
                  borderRadius: 10,
                  padding: "8px 18px",
                  fontWeight: 700,
                  fontSize: 18,
                  color: VENDOR_COLOR[data.vendor],
                  letterSpacing: 1,
                }}
              >
                {VENDOR_LABEL[data.vendor]}
              </div>
              <div>
                <div style={{ fontWeight: 600, fontSize: 15 }}>{data.name}</div>
                {data.driverKey && (
                  <div className="mono muted" style={{ fontSize: 11, marginTop: 2 }}>
                    HKLM\{data.driverKey}
                  </div>
                )}
              </div>
              <div style={{ marginLeft: "auto", textAlign: "right" }}>
                <div style={{ fontSize: 22, fontWeight: 800, color: "var(--accent)" }}>
                  {appliedCount}/{totalCount}
                </div>
                <div className="muted" style={{ fontSize: 11 }}>{t("gpuActiveCount")}</div>
              </div>
              <button className="btn ghost small" onClick={refresh} disabled={loading}>
                {loading ? <Spinner /> : "↻ Rescan"}
              </button>
            </div>

            {!data.supported && (
              <div style={{ color: "var(--yellow)", marginTop: 14, fontSize: 13 }}>
                {t("gpuNoSupport")}
              </div>
            )}

            {/* ReBAR / SAM status */}
            {data.rebar && (
              <div style={{
                marginTop: 14,
                padding: "10px 14px",
                borderRadius: 6,
                background: data.rebar.active === true
                  ? "rgba(80,200,120,0.08)"
                  : data.rebar.active === false
                  ? "rgba(255,160,40,0.08)"
                  : "var(--bg2)",
                border: `1px solid ${data.rebar.active === true ? "var(--green)" : data.rebar.active === false ? "var(--orange)" : "var(--border)"}`,
                display: "flex", gap: 12, alignItems: "center", flexWrap: "wrap",
              }}>
                <span style={{ fontSize: 18 }}>
                  {data.rebar.active === true ? "✅" : data.rebar.active === false ? "⚠️" : "❓"}
                </span>
                <div style={{ flex: 1 }}>
                  <div style={{ fontWeight: 700, fontSize: 13 }}>
                    Resizable BAR (ReBAR / SAM)
                    {data.rebar.active === true && <span style={{ color: "var(--green)", marginLeft: 8 }}>ACTIVE</span>}
                    {data.rebar.active === false && <span style={{ color: "var(--orange)", marginLeft: 8 }}>NOT ACTIVE</span>}
                  </div>
                  <div className="muted" style={{ fontSize: 11, marginTop: 2 }}>
                    {data.rebar.note}
                    {data.rebar.active === false && (
                      <span> Enable in BIOS: <b>Above 4G Decoding</b> + <b>Resizable BAR / SAM</b> → +5–15% FPS in many games.</span>
                    )}
                  </div>
                </div>
                {data.rebar.bar1Mb && (
                  <div className="muted" style={{ fontSize: 11, textAlign: "right" }}>
                    BAR1: {data.rebar.bar1Mb} MB<br />VRAM: {data.rebar.vramMb} MB
                  </div>
                )}
              </div>
            )}
          </Card>

          {/* Tweak categories */}
          {data.supported && cats.map((cat) => {
            const tweaks = applicableTweaks.filter((tw: any) => tw.category === cat);
            return (
              <div key={cat} className="mt">
                <h3
                  style={{
                    color: "var(--muted)",
                    textTransform: "uppercase",
                    fontSize: 11,
                    letterSpacing: ".5px",
                    marginBottom: 8,
                    fontWeight: 600,
                  }}
                >
                  {cat}
                </h3>
                {tweaks.map((tw) => {
                  const isBusy   = busy === tw.id;
                  const isOpen   = open === tw.id;
                  const applied  = tw.status === "applied";
                  const blocked  = tw.driverKeyMissing;
                  const needsAdmin = !admin && tw.risk === "Medium";

                  return (
                    <div className="tweak" key={tw.id} style={{ opacity: blocked ? 0.5 : 1 }}>
                      <div className="tweak-head">
                        <span
                          className="tweak-name"
                          style={{ color: applied ? VENDOR_COLOR[data.vendor] : undefined }}
                        >
                          {tw.name}
                        </span>
                        <Badge cls={`risk-${tw.risk}`}>{tw.risk}</Badge>
                        <Badge cls={STATUS_CLS[tw.status]}>{STATUS_LABEL[tw.status]}</Badge>
                        {tw.reboot && (
                          <Badge cls="st-partial">{t("gpuRebootBadge")}</Badge>
                        )}
                        <button
                          className="btn small ghost"
                          onClick={() => setOpen(isOpen ? null : tw.id)}
                        >
                          {isOpen ? "▲" : "▼"}
                        </button>
                        {/* Apply / Revert */}
                        {applied ? (
                          <button
                            className="btn small ghost"
                            disabled={isBusy || blocked}
                            onClick={() => doRevert(tw)}
                            title={blocked ? t("gpuNoKey") : ""}
                          >
                            {isBusy ? <Spinner /> : t("bootUndo")}
                          </button>
                        ) : (
                          <button
                            className="btn small"
                            disabled={isBusy || blocked || needsAdmin}
                            onClick={() => doApply(tw)}
                            title={
                              blocked    ? t("gpuNoKey") :
                              needsAdmin ? t("bootAdminNeeded") : ""
                            }
                          >
                                                   {isBusy ? <Spinner /> : t("apply")}
                          </button>
                        )}
                      </div>
                      {isOpen && (
                        <div className="tweak-desc" style={{ marginTop: 6 }}>
                          <div style={{ marginBottom: 4 }}>{tw.description}</div>
                          <div className="muted" style={{ fontSize: 12 }}>
                            {t("gpuImpact")} {tw.impact}
                          </div>
                          {tw.driverKeyMissing && (
                            <div style={{ color: "var(--orange)", fontSize: 12, marginTop: 4 }}>
                              {t("gpuNoKeyDesc")}
                            </div>
                          )}
                          {!admin && tw.risk === "Medium" && (
                            <div style={{ color: "var(--orange)", fontSize: 12, marginTop: 4 }}>
                              {t("gpuNeedsAdmin")}
                            </div>
                          )}
                        </div>
                      )}
                      {log.length > 0 && log[0].startsWith(tw.id + ":") && (
                        <div className="muted" style={{ fontSize: 11, marginTop: 4 }}>
                          {log[0].replace(tw.id + ":", "").trim()}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            );
          })}

          {!data.supported && (
            <div style={{ color: "var(--orange)", marginTop: 8 }}>
              {t("gpuNoSupport")}
            </div>
          )}
        </>
      )}
    </>
  );
}
