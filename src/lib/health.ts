// Types for the System Health Check / verified optimization engine
// (src-tauri/src/optimizer.rs + optregistry.rs).

export type Evidence =
  | "provenBeneficial"
  | "conditionallyBeneficial"
  | "systemMaintenance"
  | "noProvenBenefit"
  | "outdated"
  | "broken"
  | "potentiallyHarmful"
  | "unverified";

export type Diagnosis =
  | "configured"
  | "recommended"
  | "available"
  | "notApplicable"
  | "review"
  | "appliedNoBenefit"
  | "notRecommended"
  | "unsupported"
  | "drifted"
  | "failed"
  | "redundant"
  | "unknown";

export type Group = "gaming" | "system" | "advanced" | "privacyClutter";
export type Reboot = "notNeeded" | "signOut" | "explorerRestart" | "restart";

export type HealthItem = {
  id: string;
  name: string;
  description: string;
  risk: "Low" | "Medium" | "High" | string;
  requiresAdmin: boolean;
  canUndo: boolean;
  /** applied | partial | not_applied | unsupported | unknown */
  status: string;
  /** live = read from Windows, journal = only from the undo history */
  statusSource: "live" | "journal" | "none";
  evidence: Evidence;
  purpose: string;
  group: Group;
  benefit: string;
  when: string;
  reboot: Reboot;
  diagnosis: Diagnosis;
  reason: string;
  oneClick: boolean;
  /** Part of one-click optimization when applicable (may still be blocked). */
  oneClickCandidate: boolean;
  oneClickBlocked: string | null;
};

export type HealthCheckFinding = {
  id: string;
  severity: "warning" | "info";
  title: string;
  detail: string;
  tool: string | null;
};

export type HealthScan = {
  time: string;
  admin: boolean;
  system: {
    windows: { build: number; ubr: number; version: string; edition: string; isWindows11: boolean };
    cpu: { name: string; cores: number; threads: number; hybrid: boolean; dualCcdX3d: boolean };
    gpus: { name: string; driver: string; driverDate: string; dedicated: boolean }[];
    ramMb: number;
    isLaptop: boolean;
    storage: { hasHdd: boolean; systemDiskHdd: boolean; systemFreePct: number | null };
    power: { activePlan: string | null; highPerformanceAvailable: boolean; ultimateActive: boolean };
  };
  summary: {
    configured: number;
    recommended: number;
    available: number;
    review: number;
    failed: number;
    notApplicable: number;
    attention: number;
  };
  checks: HealthCheckFinding[];
  items: HealthItem[];
  error?: string;
};

export type ApplyOutcome = "verified" | "needsRestart" | "already" | "skipped" | "failed" | "unverified";

export type ApplyResult = {
  restorePoint: { ok: boolean; message: string } | null;
  results: { id: string; name: string; outcome: ApplyOutcome; detail: string }[];
  summary: Record<ApplyOutcome, number>;
  error?: string;
};

/** Items that need the user's attention: applied but unsuitable, drifted, failed. */
export const NEEDS_REVIEW: Diagnosis[] = ["review", "drifted", "failed"];

/** Classes that may be offered as an improvement (mirrors optregistry::offerable). */
export const OFFERABLE: Evidence[] = ["provenBeneficial", "conditionallyBeneficial", "systemMaintenance"];

const PLAN_NAMES: Record<string, string> = {
  "381b4222-f694-41f0-9685-ff5bb260df2e": "Balanced",
  "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c": "High performance",
  "a1841308-3541-4fab-bc81-f71556f20b4a": "Power saver",
};

export function planName(scan: HealthScan): string {
  const p = scan.system.power;
  if (p.ultimateActive) return "Ultimate Performance";
  return (p.activePlan && PLAN_NAMES[p.activePlan]) || "Custom";
}
