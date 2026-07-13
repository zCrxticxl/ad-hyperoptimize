import React, { useEffect, useMemo, useState } from "react";
import { api } from "../api";
import { Card, Badge, Bar } from "../components/ui";
import { useLang } from "../i18n";
import { useHwProfile } from "../hooks/useHwProfile";
import {
  GPUS, CPUS, RAM_KITS, MOTHERBOARDS, STORAGE, PSUS, buyLinks,
  Cpu, Gpu, RamKit, Motherboard, StorageDrive, Psu,
} from "../data/hardwareCatalog";
import {
  analyzeBottleneck, upgradePriority, cpuScoreOf, gpuScoreOf, ramScoreOf, storageScoreOf,
  matchCpu, matchGpu, compatibleCpus, compatibleMotherboards, compatibleRam, currentSocket,
  socketFromBoardModel, priceTotal, percentFaster, compositeOf, psuSufficient, Rating, ComponentRating,
  RATING_CLS, RATING_KEY,
} from "../lib/pcAdvisor";

const COMPONENT_KEY: Record<ComponentRating["key"], string> = {
  cpu: "pcconfComponentCpu",
  gpu: "pcconfComponentGpu",
  ram: "pcconfComponentRam",
  storage: "pcconfComponentStorage",
};
const CPU_TDP_FALLBACK: Record<string, number> = { budget: 65, mid: 105, high: 170 };
const GPU_TDP_FALLBACK: Record<string, number> = { integrated: 15, budget: 130, mid: 220, high: 350 };

type PriceablePart = { id: string; name: string; priceNew: number | null; priceUsed: number; buyQuery: string };

export default function PcConfigurator() {
  const { t, lang } = useLang();
  const hw = useHwProfile();
  const [tab, setTab] = useState<"analysis" | "configurator">("analysis");
  const [cfgMode, setCfgMode] = useState<"upgrade" | "build">("upgrade");
  const [board, setBoard] = useState<{ vendor?: string; model?: string } | null>(null);

  const [selMobo, setSelMobo] = useState("");
  const [selCpu, setSelCpu] = useState("");
  const [selGpu, setSelGpu] = useState("");
  const [selRam, setSelRam] = useState("");
  const [selStorage, setSelStorage] = useState("");
  const [selPsu, setSelPsu] = useState("");

  useEffect(() => {
    api.fullScan(false)
      .then((env: any) => setBoard({ vendor: env?.data?.board?.Manufacturer, model: env?.data?.board?.Product }))
      .catch(() => setBoard(null));
  }, []);

  // reset cross-mode-only selections so e.g. a build-mode motherboard pick
  // doesn't silently leak into upgrade mode's forced-mobo flow.
  useEffect(() => { setSelMobo(""); }, [cfgMode]);

  const bottleneck = useMemo(() => (hw ? analyzeBottleneck(hw) : null), [hw]);
  const priority = useMemo(() => (bottleneck ? upgradePriority(bottleneck) : []), [bottleneck]);

  if (!hw || !bottleneck) {
    return (
      <>
        <div className="page-title">{t("pcconfTitle")}</div>
        <div className="page-sub">{t("pcconfSub")}</div>
      </>
    );
  }

  // Prefer the socket implied by the actually-scanned motherboard (read from
  // its chipset code) over inferring it from the detected CPU name — the
  // board is the authoritative constraint for what fits, and CPU-name
  // matching can silently fail for any CPU outside our ~24-entry catalog.
  const boardSocket = socketFromBoardModel(board?.model);
  const sock = boardSocket ?? currentSocket(hw);
  const matchedCpu = matchCpu(hw.cpu.name);
  const matchedGpu = matchGpu(hw.gpu.name);
  const curCpuScore = cpuScoreOf(hw).score;
  const curGpuScore = gpuScoreOf(hw).score;
  const curRamScore = ramScoreOf(hw);
  const curStorageScore = storageScoreOf(hw);
  const inferredRamType: "DDR4" | "DDR5" = hw.ram.speedMhz >= 4000 ? "DDR5" : "DDR4";

  const cpuObj = CPUS.find((c) => c.id === selCpu) ?? null;
  const gpuObj = GPUS.find((g) => g.id === selGpu) ?? null;
  const ramObj = RAM_KITS.find((r) => r.id === selRam) ?? null;
  const storageObj = STORAGE.find((s) => s.id === selStorage) ?? null;
  const psuObj = PSUS.find((p) => p.id === selPsu) ?? null;
  const buildMoboObj = MOTHERBOARDS.find((m) => m.id === selMobo) ?? null;

  const needsNewMobo = cfgMode === "upgrade" && !!cpuObj && !!sock && cpuObj.socket !== sock;
  const forcedMoboOptions = needsNewMobo ? compatibleMotherboards(cpuObj!.socket) : [];
  const forcedMoboObj = needsNewMobo ? MOTHERBOARDS.find((m) => m.id === selMobo) ?? null : null;
  const effectiveMobo = cfgMode === "build" ? buildMoboObj : forcedMoboObj;

  const cpuOptions: Cpu[] = cfgMode === "build" ? (buildMoboObj ? compatibleCpus(buildMoboObj.socket) : []) : CPUS;
  const effectiveRamType: "DDR4" | "DDR5" =
    cfgMode === "build" ? (buildMoboObj?.ramType ?? "DDR5") : effectiveMobo?.ramType ?? inferredRamType;
  const ramOptions: RamKit[] = compatibleRam(effectiveRamType);

  const effCpuScore = cpuObj?.score ?? curCpuScore;
  const effGpuScore = gpuObj?.score ?? curGpuScore;
  const effRamScore = ramObj?.score ?? curRamScore;
  const effStorageScore = storageObj?.score ?? curStorageScore;
  const newComposite = compositeOf(effCpuScore, effGpuScore, effRamScore, effStorageScore);
  const speedDelta = percentFaster(bottleneck.compositeScore, newComposite);

  const includeMobo = cfgMode === "build" ? !!buildMoboObj : needsNewMobo;
  const priceParts: PriceablePart[] = [
    cpuObj, gpuObj, ramObj, storageObj, psuObj, includeMobo ? effectiveMobo : null,
  ].filter(Boolean) as unknown as PriceablePart[];
  const totals = priceTotal(priceParts);

  const effCpuTdp = cpuObj?.tdpW ?? matchedCpu?.tdpW ?? CPU_TDP_FALLBACK[hw.cpu.tier] ?? 105;
  const effGpuTdp = hw.gpu.isIntegrated && !gpuObj
    ? GPU_TDP_FALLBACK.integrated
    : gpuObj?.tdpW ?? matchedGpu?.tdpW ?? GPU_TDP_FALLBACK[hw.gpu.tier] ?? 220;
  const psuOk = psuObj ? psuSufficient(effCpuTdp + effGpuTdp, psuObj.watts) : null;

  function nextUpgrade<T extends { score: number }>(list: T[], currentScore: number): T | null {
    const better = list.filter((x) => x.score > currentScore + 8);
    if (!better.length) return null;
    return better.reduce((a, b) => (b.score < a.score ? b : a));
  }

  function BuyButtons({ query }: { query: string }) {
    return (
      <div className="row" style={{ gap: 6, marginTop: 4 }}>
        {buyLinks(query, lang).map((l) => (
          <button key={l.label} className="btn ghost small" onClick={() => api.openPath(l.url)}>
            {l.label}
          </button>
        ))}
      </div>
    );
  }

  function PriceTag({ part }: { part: PriceablePart }) {
    return (
      <div style={{ marginTop: 6, fontSize: 13 }}>
        <span>{t("pcconfPriceNew")}: {part.priceNew != null ? `$${part.priceNew}` : t("pcconfNoLongerSoldNew")}</span>
        {"  ·  "}
        <span>{t("pcconfPriceUsed")}: ${part.priceUsed}</span>
        <BuyButtons query={part.buyQuery} />
      </div>
    );
  }

  function PartSelect<T extends PriceablePart>({
    label, options, value, onChange,
  }: { label: string; options: T[]; value: string; onChange: (v: string) => void }) {
    const sel = options.find((o) => o.id === value) ?? null;
    return (
      <div style={{ marginBottom: 16 }}>
        <div className="muted" style={{ fontSize: 12, marginBottom: 4 }}>{label}</div>
        <select className="select" value={value} onChange={(e) => onChange(e.target.value)} disabled={options.length === 0}>
          <option value="">{t("pcconfNone")}</option>
          {options.map((o) => (
            <option key={o.id} value={o.id}>{o.name}</option>
          ))}
        </select>
        {options.length === 0 && <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>{t("pcconfNoOptions")}</div>}
        {sel && <PriceTag part={sel} />}
      </div>
    );
  }

  function ComponentRow({ cr }: { cr: ComponentRating }) {
    return (
      <div style={{ marginBottom: 14 }}>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <span>{t(COMPONENT_KEY[cr.key] as any)}</span>
          <Badge cls={RATING_CLS[cr.rating]}>{t(RATING_KEY[cr.rating] as any)}</Badge>
        </div>
        <Bar pct={cr.score} />
        <div className="muted" style={{ fontSize: 12, marginTop: 3 }}>{t("pcconfScoreLabel")}: {Math.round(cr.score)}/100</div>
      </div>
    );
  }

  return (
    <>
      <div className="page-title">{t("pcconfTitle")}</div>
      <div className="page-sub">{t("pcconfSub")}</div>

      <div className="mode-toggle" style={{ margin: "14px 0", width: 360 }}>
        <button className={tab === "analysis" ? "on" : ""} onClick={() => setTab("analysis")}>{t("pcconfTabAnalysis")}</button>
        <button className={tab === "configurator" ? "on" : ""} onClick={() => setTab("configurator")}>{t("pcconfTabConfigurator")}</button>
      </div>

      {tab === "analysis" && (
        <>
          <div className="grid grid-2">
            <Card title={t("pcconfYourSystem")}>
              {bottleneck.components.map((cr) => <ComponentRow key={cr.key} cr={cr} />)}
            </Card>
            <Card title={t("pcconfOverallScore")}>
              <div className="stat-big">{Math.round(bottleneck.compositeScore)}/100</div>
              <Bar pct={bottleneck.compositeScore} />
              <div style={{ marginTop: 14 }}>
                <span className="muted">{t("pcconfWeakestLink")}: </span>
                <Badge cls={RATING_CLS[bottleneck.weakest.rating]}>{t(COMPONENT_KEY[bottleneck.weakest.key] as any)}</Badge>
              </div>
              <div className="muted" style={{ marginTop: 10, fontSize: 13 }}>
                {t(
                  (bottleneck.cpuGpuImbalance === "cpu_limited"
                    ? "pcconfImbalanceCpuLimited"
                    : bottleneck.cpuGpuImbalance === "gpu_limited"
                    ? "pcconfImbalanceGpuLimited"
                    : "pcconfImbalanceBalanced") as any
                )}
              </div>
            </Card>
          </div>

          <Card title={t("pcconfPriorityTitle")} style={{ marginTop: 14 }}>
            <div className="muted" style={{ fontSize: 13, marginBottom: 10 }}>{t("pcconfPriorityNote")}</div>
            {priority.map((cr, i) => {
              const isCpu = cr.key === "cpu";
              const isGpu = cr.key === "gpu";
              const sameSocketCpus = sock ? compatibleCpus(sock) : CPUS;
              const suggestion = isCpu
                ? nextUpgrade(sameSocketCpus, curCpuScore)
                : isGpu
                ? nextUpgrade(GPUS, curGpuScore)
                : null;
              return (
                <div key={cr.key} style={{ padding: "10px 0", borderBottom: i < priority.length - 1 ? "1px solid var(--border)" : "none" }}>
                  <div className="row" style={{ justifyContent: "space-between" }}>
                    <span><b>{i + 1}.</b> {t(COMPONENT_KEY[cr.key] as any)}</span>
                    <Badge cls={RATING_CLS[cr.rating]}>{t(RATING_KEY[cr.rating] as any)}</Badge>
                  </div>
                  {(isCpu || isGpu) && (
                    suggestion ? (
                      <div style={{ marginTop: 6 }}>
                        <span className="muted">{t("pcconfUpgradeTo")}: </span>
                        <b>{suggestion.name}</b> ({t("pcconfScoreLabel")} {Math.round(suggestion.score)})
                        <PriceTag part={suggestion as unknown as PriceablePart} />
                      </div>
                    ) : (
                      <div className="muted" style={{ marginTop: 6, fontSize: 13 }}>
                        {cr.rating === "excellent" || cr.rating === "good" ? t("pcconfNoUpgradeNeeded") : t("pcconfRequiresNewMobo")}
                      </div>
                    )
                  )}
                </div>
              );
            })}
          </Card>
        </>
      )}

      {tab === "configurator" && (
        <>
          <div className="mode-toggle" style={{ margin: "0 0 14px", width: 320 }}>
            <button className={cfgMode === "upgrade" ? "on" : ""} onClick={() => setCfgMode("upgrade")}>{t("pcconfModeUpgrade")}</button>
            <button className={cfgMode === "build" ? "on" : ""} onClick={() => setCfgMode("build")}>{t("pcconfModeBuild")}</button>
          </div>

          {cfgMode === "upgrade" && (
            <div className="muted" style={{ fontSize: 13, marginBottom: 14 }}>
              {board?.model && <>{t("pcconfBoardDetected")}: {board.vendor} {board.model} · </>}
              {sock ? `${t("pcconfDetectedAs")}: ${sock}` : t("pcconfSocketUnknownWarning")}
            </div>
          )}

          <div className="grid grid-2">
            <Card title={t("pcconfTabConfigurator")}>
              {cfgMode === "build" && (
                <PartSelect label={t("pcconfSelectMotherboard")} options={MOTHERBOARDS as unknown as PriceablePart[]} value={selMobo} onChange={setSelMobo} />
              )}

              <div className="muted" style={{ fontSize: 12, marginBottom: 6 }}>
                {t("pcconfCurrentPart")}: {matchedCpu?.name ?? hw.cpu.name} ({t("pcconfScoreLabel")} {Math.round(curCpuScore)})
                {!matchedCpu && <span> — {t("pcconfNoExactMatch")}</span>}
              </div>
              <PartSelect label={t("pcconfSelectCpu")} options={cpuOptions as unknown as PriceablePart[]} value={selCpu} onChange={setSelCpu} />
              {cfgMode === "upgrade" && cpuObj && (
                needsNewMobo ? (
                  <div style={{ marginBottom: 16 }}>
                    <Badge cls="risk-High">{t("pcconfRequiresNewMobo")}</Badge>
                    <div style={{ marginTop: 8 }}>
                      <PartSelect label={t("pcconfSelectMotherboard")} options={forcedMoboOptions as unknown as PriceablePart[]} value={selMobo} onChange={setSelMobo} />
                    </div>
                  </div>
                ) : sock ? (
                  <div style={{ marginBottom: 16 }}>
                    <Badge cls="st-applied">{t("pcconfCpuCompatible")}</Badge>
                  </div>
                ) : (
                  <div style={{ marginBottom: 16 }}>
                    <Badge cls="risk-Medium">{t("pcconfSocketUnknownWarning")}</Badge>
                  </div>
                )
              )}

              <div className="muted" style={{ fontSize: 12, marginBottom: 6 }}>
                {t("pcconfCurrentPart")}: {matchedGpu?.name ?? hw.gpu.name} ({t("pcconfScoreLabel")} {Math.round(curGpuScore)})
                {!matchedGpu && <span> — {t("pcconfNoExactMatch")}</span>}
              </div>
              <PartSelect label={t("pcconfSelectGpu")} options={GPUS as unknown as PriceablePart[]} value={selGpu} onChange={setSelGpu} />

              <PartSelect label={t("pcconfSelectRam")} options={ramOptions as unknown as PriceablePart[]} value={selRam} onChange={setSelRam} />
              <div className="muted" style={{ fontSize: 12, marginTop: -10, marginBottom: 16 }}>⚠ {t("pcconfRamShortageNote")}</div>
              <PartSelect label={t("pcconfSelectStorage")} options={STORAGE as unknown as PriceablePart[]} value={selStorage} onChange={setSelStorage} />
              <PartSelect label={t("pcconfSelectPsu")} options={PSUS as unknown as PriceablePart[]} value={selPsu} onChange={setSelPsu} />
              {psuOk !== null && (
                <Badge cls={psuOk ? "st-applied" : "risk-High"}>{psuOk ? t("pcconfPsuOk") : t("pcconfPsuInsufficient")}</Badge>
              )}

              <div className="muted" style={{ fontSize: 12, marginTop: 16 }}>{t("pcconfCatalogNote")}</div>
              <div className="muted" style={{ fontSize: 12, marginTop: 4 }}>{t("pcconfPriceDisclaimer")}</div>
            </Card>

            <div>
              <Card title={t("pcconfEstimatedPerf")}>
                <div className="row" style={{ gap: 24 }}>
                  <div style={{ flex: 1 }}>
                    <div className="muted" style={{ fontSize: 12 }}>{t("pcconfYourSystem")}</div>
                    <div className="stat-big">{Math.round(bottleneck.compositeScore)}</div>
                    <Bar pct={bottleneck.compositeScore} />
                  </div>
                  <div style={{ flex: 1 }}>
                    <div className="muted" style={{ fontSize: 12 }}>{t("pcconfTabConfigurator")}</div>
                    <div className="stat-big" style={{ color: speedDelta >= 0 ? "var(--green)" : "var(--red)" }}>{Math.round(newComposite)}</div>
                    <Bar pct={newComposite} />
                  </div>
                </div>
                <div style={{ marginTop: 14, fontSize: 15 }}>
                  <b style={{ color: speedDelta > 0 ? "var(--green)" : speedDelta < 0 ? "var(--red)" : "var(--text)" }}>
                    {speedDelta > 0 ? `+${speedDelta}%` : `${speedDelta}%`}
                  </b>{" "}
                  {speedDelta > 0 ? t("pcconfFasterThanCurrent") : speedDelta < 0 ? t("pcconfSlowerThanCurrent") : t("pcconfSameAsCurrent")}
                </div>

                <div style={{ marginTop: 16, fontSize: 12 }}>
                  <div className="muted" style={{ marginBottom: 6 }}>{t("pcconfBreakdownTitle")}</div>
                  {[
                    { key: "gpu", label: t("pcconfComponentGpu"), weight: 55, cur: curGpuScore, eff: effGpuScore },
                    { key: "cpu", label: t("pcconfComponentCpu"), weight: 30, cur: curCpuScore, eff: effCpuScore },
                    { key: "ram", label: t("pcconfComponentRam"), weight: 10, cur: curRamScore, eff: effRamScore },
                    { key: "storage", label: t("pcconfComponentStorage"), weight: 5, cur: curStorageScore, eff: effStorageScore },
                  ].map((c) => (
                    <div key={c.key} className="row" style={{ justifyContent: "space-between", padding: "3px 0" }}>
                      <span>{c.label} ({c.weight}%)</span>
                      <span>
                        {Math.round(c.cur)} →{" "}
                        <b style={{ color: c.eff > c.cur ? "var(--green)" : c.eff < c.cur ? "var(--red)" : "var(--text)" }}>
                          {Math.round(c.eff)}
                        </b>
                      </span>
                    </div>
                  ))}
                  <div className="muted" style={{ marginTop: 8 }}>{t("pcconfCompositeNote")}</div>
                </div>
              </Card>

              <Card title={t("pcconfTotalCost")} style={{ marginTop: 14 }}>
                <table className="tbl">
                  <tbody>
                    <tr><td className="muted">{t("pcconfTotalNew")}</td><td><b>{totals.newTotal != null ? `$${totals.newTotal}` : t("pcconfNoLongerSoldNew")}</b></td></tr>
                    <tr><td className="muted">{t("pcconfTotalUsed")}</td><td><b>${totals.usedTotal}</b></td></tr>
                  </tbody>
                </table>
              </Card>
            </div>
          </div>
        </>
      )}
    </>
  );
}
