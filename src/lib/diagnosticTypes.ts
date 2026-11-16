export type RecoverySetting = { kind: "Registry"; value: { t: string; v: unknown } | null } | { kind: "Service"; value: string };
export type RecoveryItem = { resource: string; before: RecoverySetting | null; target: RecoverySetting | null; current: RecoverySetting | null; error?: string | null; status: string };
export type RecoveryEntry = { id: string; name: string; status: string; restored: number; attempted: number; items: RecoveryItem[] };
export type RecoverySession = { id: string; label: string; time: string; legacy: boolean; entries: RecoveryEntry[] };
export type NetworkStat = { target: string; sent: number; received: number; errors: number; lossPct: number | null; avgMs: number | null; jitterMs: number | null; maxMs: number | null };
export type NetworkPhase = { seconds: number; rxMbps: number; txMbps: number; stats: NetworkStat[] };
export type NetworkResult = { adapter: string; description: string; gateway: string; linkSpeed: string; wifiDetails: string | null; wifiError: string | null; measurements: { idle: NetworkPhase; loaded: NetworkPhase | null; downloadBytes: number; downloadSeconds: number; loadError: string | null; byteLimitReached: boolean } };
