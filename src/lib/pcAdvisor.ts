// ─────────────────────────────────────────────────────────────────────────────
// Bottleneck analysis + PC configurator logic. Pure functions over the
// existing hw_profile() shape (cpu/gpu/ram/storage + tier fields already
// computed by hwprofile.rs) and the static catalog in data/hardwareCatalog.ts.
// No new Rust/backend surface needed — everything here is derived client-side.
// ─────────────────────────────────────────────────────────────────────────────

import { CPUS, GPUS, RAM_KITS, MOTHERBOARDS, Cpu, Gpu, RamKit, Motherboard } from "../data/hardwareCatalog";

export type Rating = "bottleneck" | "needsImprovement" | "good" | "excellent";

export function ratingFromScore(score: number): Rating {
  if (score < 30) return "bottleneck";
  if (score < 55) return "needsImprovement";
  if (score < 80) return "good";
  return "excellent";
}

/** GPU scores are TechPowerUp Relative-Performance % (raw throughput vs. the
 * current flagship), not a "gaming adequacy" curve — unlike CPU/RAM/storage,
 * where the source benchmarks already compress gently enough that the flat
 * 30/55/80 split tracks real-world adequacy. Applying that same flat split to
 * GPU would call a 19%-of-RTX-5090 RTX 3060 — still a perfectly playable
 * 1080p/1440p card — a "bottleneck" purely for not being within ~2 gens of
 * the flagship. These thresholds are recalibrated against where real-world
 * playability actually falls off (sub-15%: legacy/weak even at 1080p today;
 * 15-25%: fine at 1080p, weak at 1440p+; 25-42%: solid 1440p; 42%+: strong
 * 1440p/4K). */
export function ratingFromGpuScore(score: number): Rating {
  if (score < 15) return "bottleneck";
  if (score < 25) return "needsImprovement";
  if (score < 42) return "good";
  return "excellent";
}

// UI mapping shared by every screen that renders a `Rating` badge (Dashboard's
// HwProfileCard, PcConfigurator) — keeps badge color/label consistent app-wide.
export const RATING_CLS: Record<Rating, string> = {
  bottleneck: "risk-High",
  needsImprovement: "risk-Medium",
  good: "sev-2",
  excellent: "st-applied",
};
export const RATING_KEY: Record<Rating, string> = {
  bottleneck: "pcconfRatingBottleneck",
  needsImprovement: "pcconfRatingNeedsImprovement",
  good: "pcconfRatingGood",
  excellent: "pcconfRatingExcellent",
};

// ── name normalization + fuzzy catalog matching ───────────────────────────────
function normalize(s: string): string {
  return (s || "")
    .toLowerCase()
    .replace(/\(r\)|\(tm\)|\(c\)/g, "")
    .replace(/w\/\s*radeon graphics/g, "")
    .replace(/cpu|processor|graphics|geforce|radeon(?!\s*rx)/g, "")
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

export function matchCpu(detectedName: string): Cpu | null {
  const n = normalize(detectedName);
  if (!n) return null;
  // Exact normalized match always wins outright. Without this, the
  // longest-match fallback below would let a longer catalog name (e.g.
  // "Ryzen 7 5800X3D") win over a true exact match ("Ryzen 7 5800X") just
  // because "5800x" is a textual prefix of "5800x3d" — a real bug found
  // while auditing the catalog (same pattern hits RTX/GTX/RX "base" models
  // vs. their Ti/Super/XT/X3D siblings).
  for (const c of CPUS) {
    if (normalize(c.name) === n) return c;
  }
  let best: Cpu | null = null;
  let bestLen = 0;
  for (const c of CPUS) {
    const cn = normalize(c.name);
    if ((n.includes(cn) || cn.includes(n)) && cn.length > bestLen) {
      best = c;
      bestLen = cn.length;
    }
  }
  return best;
}

export function matchGpu(detectedName: string): Gpu | null {
  const n = normalize(detectedName);
  if (!n) return null;
  // See matchCpu: exact match must win before the longest-substring
  // fallback, otherwise e.g. a bare "RTX 4070" detection incorrectly
  // matches the longer "RTX 4070 Ti" / "RTX 4070 Ti Super" catalog entries.
  for (const g of GPUS) {
    if (normalize(g.name) === n) return g;
  }
  let best: Gpu | null = null;
  let bestLen = 0;
  for (const g of GPUS) {
    const gn = normalize(g.name);
    if ((n.includes(gn) || gn.includes(n)) && gn.length > bestLen) {
      best = g;
      bestLen = gn.length;
    }
  }
  return best;
}

// ── fallback scoring when no exact catalog match is found ───────────────────
const CPU_TIER_FALLBACK: Record<string, number> = { budget: 25, mid: 50, high: 80 };
const GPU_TIER_FALLBACK: Record<string, number> = { integrated: 8, budget: 22, mid: 45, high: 78 };

export interface HwProfileLike {
  cpu: { name: string; cores: number; threads: number; tier?: string };
  gpu: { name: string; vramMb: number; tier?: string; isIntegrated?: boolean };
  ram: { totalMb: number; speedMhz: number; tier?: string };
  storage: { tier?: string; hasNvme?: boolean; hasSsd?: boolean; hasHdd?: boolean };
}

export function cpuScoreOf(hw: HwProfileLike): { score: number; match: Cpu | null } {
  const match = matchCpu(hw.cpu?.name ?? "");
  if (match) return { score: match.score, match };
  return { score: CPU_TIER_FALLBACK[hw.cpu?.tier ?? "mid"] ?? 45, match: null };
}

export function gpuScoreOf(hw: HwProfileLike): { score: number; match: Gpu | null } {
  if (hw.gpu?.isIntegrated) return { score: GPU_TIER_FALLBACK.integrated, match: null };
  const match = matchGpu(hw.gpu?.name ?? "");
  if (match) return { score: match.score, match };
  return { score: GPU_TIER_FALLBACK[hw.gpu?.tier ?? "mid"] ?? 40, match: null };
}

export function ramScoreOf(hw: HwProfileLike): number {
  const gb = (hw.ram?.totalMb ?? 0) / 1024;
  let score = gb < 8 ? 12 : gb < 16 ? 42 : gb < 32 ? 72 : gb < 64 ? 90 : 97;
  const speed = hw.ram?.speedMhz ?? 0;
  if (speed > 0 && speed < 2400) score -= 8;
  else if (speed >= 5600) score += 4;
  return Math.max(0, Math.min(100, score));
}

export function storageScoreOf(hw: HwProfileLike): number {
  const tier = hw.storage?.tier;
  if (tier === "nvme") return 88;
  if (tier === "sata_ssd") return 55;
  if (tier === "hdd") return 18;
  return 40;
}

// ── component rating bundle ───────────────────────────────────────────────────
export interface ComponentRating {
  key: "cpu" | "gpu" | "ram" | "storage";
  score: number;
  rating: Rating;
  weight: number; // relative importance for gaming priority ranking
}

export interface BottleneckResult {
  components: ComponentRating[];
  weakest: ComponentRating;
  compositeScore: number;
  cpuGpuImbalance: "cpu_limited" | "gpu_limited" | "balanced";
}

const WEIGHTS = { gpu: 0.55, cpu: 0.30, ram: 0.10, storage: 0.05 };

export function analyzeBottleneck(hw: HwProfileLike): BottleneckResult {
  const { score: cpuScore } = cpuScoreOf(hw);
  const { score: gpuScore } = gpuScoreOf(hw);
  const ramScore = ramScoreOf(hw);
  const storageScore = storageScoreOf(hw);

  const components: ComponentRating[] = [
    { key: "gpu", score: gpuScore, rating: ratingFromGpuScore(gpuScore), weight: WEIGHTS.gpu },
    { key: "cpu", score: cpuScore, rating: ratingFromScore(cpuScore), weight: WEIGHTS.cpu },
    { key: "ram", score: ramScore, rating: ratingFromScore(ramScore), weight: WEIGHTS.ram },
    { key: "storage", score: storageScore, rating: ratingFromScore(storageScore), weight: WEIGHTS.storage },
  ];

  const weakest = components.reduce((a, b) => (b.score < a.score ? b : a));

  const compositeScore =
    gpuScore * WEIGHTS.gpu + cpuScore * WEIGHTS.cpu + ramScore * WEIGHTS.ram + storageScore * WEIGHTS.storage;

  let cpuGpuImbalance: BottleneckResult["cpuGpuImbalance"] = "balanced";
  if (cpuScore < gpuScore * 0.7) cpuGpuImbalance = "cpu_limited";
  else if (gpuScore < cpuScore * 0.6) cpuGpuImbalance = "gpu_limited";

  return { components, weakest, compositeScore, cpuGpuImbalance };
}

/** Upgrade priority order: worst rating first, gaming-weight as tiebreak. */
export function upgradePriority(result: BottleneckResult): ComponentRating[] {
  return [...result.components].sort((a, b) => {
    const order: Record<Rating, number> = { bottleneck: 0, needsImprovement: 1, good: 2, excellent: 3 };
    if (order[a.rating] !== order[b.rating]) return order[a.rating] - order[b.rating];
    return b.weight - a.weight;
  });
}

// ── compatibility helpers ─────────────────────────────────────────────────────
export function compatibleCpus(socket: string): Cpu[] {
  return CPUS.filter((c) => c.socket === socket);
}
export function compatibleMotherboards(socket: string): Motherboard[] {
  return MOTHERBOARDS.filter((m) => m.socket === socket);
}
export function compatibleRam(ramType: "DDR4" | "DDR5"): RamKit[] {
  return RAM_KITS.filter((r) => r.type === ramType);
}

/** Best-guess current socket from the detected CPU name — used to filter
 * "drop-in" upgrade candidates without requiring motherboard model lookup. */
export function currentSocket(hw: HwProfileLike): string | null {
  return matchCpu(hw.cpu?.name ?? "")?.socket ?? null;
}

// Chipset code → socket. Real-world motherboard product names (from WMI
// Win32_BaseBoard) always embed the chipset code (e.g. "ROG STRIX B650E-F
// GAMING WIFI", "PRIME Z790-A"), so this generalizes far beyond the ~10
// placeholder boards in MOTHERBOARDS and is the most authoritative signal
// we have — it reads the *actual* board, not an inference from the CPU.
const CHIPSET_SOCKET_MAP: { re: RegExp; socket: string }[] = [
  { re: /\b(A620|B650E?|X670E?|X870E?)\b/i, socket: "AM5" },
  { re: /\b(A320|A520|B350|B450|B550|X370|X470|X570)\b/i, socket: "AM4" },
  { re: /\b(B860|Z890)\b/i, socket: "LGA1851" },
  { re: /\b(H610|H670|B660|B760|H770|Z690|Z790)\b/i, socket: "LGA1700" },
  { re: /\b(H410|B460|H470|Z490|H510|B560|H570|Z590)\b/i, socket: "LGA1200" },
  { re: /\b(H310|B360|H370|Q370|Z370|B365|Z390)\b/i, socket: "LGA1151" },
];

/** Derive socket directly from a scanned motherboard product name via its
 * chipset code. Returns null if no known chipset pattern is found — callers
 * must treat null as "compatibility unverifiable", never as "compatible". */
export function socketFromBoardModel(model: string | undefined | null): string | null {
  if (!model) return null;
  for (const { re, socket } of CHIPSET_SOCKET_MAP) {
    if (re.test(model)) return socket;
  }
  return null;
}

// ── pricing ────────────────────────────────────────────────────────────────────
export function priceTotal(parts: { priceNew: number | null; priceUsed: number }[]): {
  newTotal: number | null;
  usedTotal: number;
} {
  let newTotal: number | null = 0;
  let usedTotal = 0;
  for (const p of parts) {
    usedTotal += p.priceUsed;
    if (p.priceNew == null) newTotal = null;
    else if (newTotal != null) newTotal += p.priceNew;
  }
  return { newTotal, usedTotal };
}

/** % faster of a candidate build vs. the currently detected system, using the
 * same gaming-weighted composite as analyzeBottleneck. */
export function percentFaster(currentComposite: number, candidateComposite: number): number {
  if (currentComposite <= 0) return 0;
  return Math.round(((candidateComposite - currentComposite) / currentComposite) * 100);
}

export function compositeOf(cpuScore: number, gpuScore: number, ramScore: number, storageScore: number): number {
  return cpuScore * WEIGHTS.cpu + gpuScore * WEIGHTS.gpu + ramScore * WEIGHTS.ram + storageScore * WEIGHTS.storage;
}

/** Rough PSU sufficiency check: sum of part TDPs + ~150W headroom for
 * mainboard/storage/fans, compared against the PSU's rated wattage. */
export function psuSufficient(totalTdpW: number, psuWatts: number): boolean {
  return psuWatts >= totalTdpW + 150;
}
