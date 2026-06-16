import React, { useState } from "react";
import { useHwWarnings, useHwProfile, useTweakRisk, HwWarning, HwProfile, TweakRisk } from "../hooks/useHwProfile";

// ─── severity styling ────────────────────────────────────────────────────────
const SEV: Record<string, { bg: string; border: string; icon: string; label: string }> = {
  info:    { bg: "rgba(0,140,255,0.07)",   border: "var(--accent)",  icon: "ℹ", label: "INFO"    },
  warning: { bg: "rgba(255,165,0,0.09)",   border: "var(--orange)",  icon: "⚠", label: "WARNING" },
  danger:  { bg: "rgba(220,50,50,0.10)",   border: "var(--red)",     icon: "✕", label: "ISSUE"   },
};

// ─── single collapsible banner ───────────────────────────────────────────────
function WarningBanner({ w }: { w: HwWarning }) {
  const [open, setOpen] = useState(false);
  const s = SEV[w.severity] ?? SEV.info;
  return (
    <div
      style={{
        background:   s.bg,
        border:       `1px solid ${s.border}`,
        borderRadius: 6,
        padding:      "8px 12px",
        marginBottom: 6,
        cursor:       "pointer",
        userSelect:   "none",
      }}
      onClick={() => setOpen(o => !o)}
    >
      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
        <span style={{ color: s.border, fontWeight: 700, fontSize: 13, minWidth: 14 }}>{s.icon}</span>
        <span style={{ fontWeight: 600, fontSize: 12, color: s.border }}>{s.label}</span>
        <span style={{ fontWeight: 600, fontSize: 12 }}>{w.title}</span>
        <span style={{ marginLeft: "auto", fontSize: 10, opacity: 0.45 }}>{open ? "▲" : "▼"}</span>
      </div>
      {open && (
        <div style={{ marginTop: 6, fontSize: 12, lineHeight: 1.55, opacity: 0.82, paddingLeft: 22 }}>
          {w.message}
        </div>
      )}
    </div>
  );
}

// ─── main export: stacked banners for a page ────────────────────────────────
export function HwWarnings({ page }: { page: string }) {
  const warnings = useHwWarnings(page);
  if (!warnings.length) return null;
  return (
    <div style={{ marginBottom: 14 }}>
      {warnings.map(w => <WarningBanner key={w.id} w={w} />)}
    </div>
  );
}

// ─── compact chip row (for headers / inline use) ─────────────────────────────
export function HwWarningChips({ page }: { page: string }) {
  const warnings = useHwWarnings(page);
  if (!warnings.length) return null;
  return (
    <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 10 }}>
      {warnings.map(w => {
        const s = SEV[w.severity] ?? SEV.info;
        return (
          <span
            key={w.id}
            title={w.message}
            style={{
              fontSize:     11,
              padding:      "2px 8px",
              borderRadius: 12,
              background:   s.bg,
              border:       `1px solid ${s.border}`,
              color:        s.border,
              fontWeight:   600,
              cursor:       "help",
            }}
          >
            {s.icon} {w.title}
          </span>
        );
      })}
    </div>
  );
}

// ─── per-tweak hardware risk badge + block-on-apply gate ─────────────────────
// Use anywhere a tweak/setting has an `id` that may appear in profile.tweakRisks.

const OK_STYLE = { bg: "rgba(80,200,120,0.08)", border: "var(--green)" };

export function RiskBadge({ id }: { id: string }) {
  const profile = useHwProfile();
  const risk = useTweakRisk(id);

  // Hardware not detected yet — don't claim a verdict we haven't checked.
  if (!profile) return null;

  if (!risk) {
    return (
      <span
        title="No known issue for your detected hardware"
        style={{
          fontSize: 10.5,
          fontWeight: 700,
          padding: "1px 7px",
          borderRadius: 10,
          background: OK_STYLE.bg,
          border: `1px solid ${OK_STYLE.border}`,
          color: OK_STYLE.border,
          cursor: "help",
          whiteSpace: "nowrap",
        }}
      >
        ✓ OK FOR YOUR HARDWARE
      </span>
    );
  }

  const s = SEV[risk.severity] ?? SEV.warning;
  return (
    <span
      title={risk.message}
      style={{
        fontSize: 10.5,
        fontWeight: 700,
        padding: "1px 7px",
        borderRadius: 10,
        background: s.bg,
        border: `1px solid ${s.border}`,
        color: s.border,
        cursor: "help",
        whiteSpace: "nowrap",
      }}
    >
      {s.icon} {risk.severity === "danger" ? "RISKY FOR YOUR HARDWARE" : "CHECK YOUR HARDWARE"}
    </span>
  );
}

// Inline block shown inside an open detail/confirm panel — always renders the
// full risk message (not just a tooltip) so the user cannot miss it before confirming.
export function RiskNotice({ id }: { id: string }) {
  const risk = useTweakRisk(id);
  if (!risk) return null;
  const s = SEV[risk.severity] ?? SEV.warning;
  return (
    <div
      style={{
        marginTop: 8,
        padding: "8px 10px",
        borderRadius: 6,
        background: s.bg,
        border: `1px solid ${s.border}`,
        fontSize: 12,
        lineHeight: 1.5,
      }}
    >
      <b style={{ color: s.border }}>{s.icon} {risk.title}</b>
      <div style={{ marginTop: 3, opacity: 0.88 }}>{risk.message}</div>
    </div>
  );
}

// Hook: returns whether a tweak id requires an explicit "apply anyway"
// confirmation before it's allowed to run (severity === "danger").
export function useRequiresRiskConfirm(id: string): boolean {
  const risk = useTweakRisk(id);
  return risk?.severity === "danger";
}

// ─── hardware summary card (for Dashboard) ───────────────────────────────────
const TIER_COLOR: Record<string, string> = {
  high:      "var(--green)",
  mid:       "var(--yellow)",
  ok:        "var(--green)",
  good:      "var(--green)",
  nvme:      "var(--green)",
  sata_ssd:  "var(--yellow)",
  budget:    "var(--orange)",
  low:       "var(--red)",
  hdd:       "var(--red)",
  integrated:"var(--red)",
  unknown:   "var(--muted)",
};

function TierBadge({ tier }: { tier: string }) {
  const color = TIER_COLOR[tier] ?? "var(--muted)";
  return (
    <span style={{
      fontSize: 10, fontWeight: 700, textTransform: "uppercase",
      padding: "1px 7px", borderRadius: 10,
      background: `${color}22`, color, border: `1px solid ${color}`,
      marginLeft: 6,
    }}>
      {tier.replace("_", " ")}
    </span>
  );
}

function Row({ label, value, tier }: { label: string; value: string; tier?: string }) {
  return (
    <div style={{ display: "flex", alignItems: "center", padding: "5px 0", borderBottom: "1px solid var(--border)", fontSize: 12, gap: 8 }}>
      <span className="muted" style={{ minWidth: 80 }}>{label}</span>
      <span style={{ flex: 1, fontWeight: 500 }}>{value}</span>
      {tier && <TierBadge tier={tier} />}
    </div>
  );
}

function fmtRam(mb: number) {
  if (!mb) return "—";
  return mb >= 1024 ? `${(mb / 1024).toFixed(0)} GB` : `${mb} MB`;
}

export function HwProfileCard() {
  const p: HwProfile | null = useHwProfile();
  if (!p) return <div className="muted" style={{ fontSize: 12 }}>Detecting hardware…</div>;

  const storageLabel = p.storage.hasNvme
    ? "NVMe SSD" + (p.storage.hasHdd ? " + HDD" : "")
    : p.storage.hasSsd
    ? "SATA SSD" + (p.storage.hasHdd ? " + HDD" : "")
    : p.storage.hasHdd
    ? "HDD only"
    : "Unknown";

  return (
    <div>
      <Row label="CPU" value={p.cpu.name} tier={p.cpu.tier} />
      <Row label="GPU" value={`${p.gpu.name}${p.gpu.vramMb ? ` · ${p.gpu.vramMb >= 1024 ? (p.gpu.vramMb / 1024).toFixed(0) + " GB" : p.gpu.vramMb + " MB"} VRAM` : ""}`} tier={p.gpu.tier} />
      <Row label="RAM" value={`${fmtRam(p.ram.totalMb)}${p.ram.speedMhz ? ` · ${p.ram.speedMhz} MHz` : ""} · ${p.ram.sticks} stick${p.ram.sticks !== 1 ? "s" : ""}`} tier={p.ram.tier} />
      <Row label="Storage" value={storageLabel} tier={p.storage.tier} />
      {p.isLaptop && <Row label="Platform" value="Laptop" />}
      {p.isWifi && <Row label="Network" value="Wi-Fi" />}
    </div>
  );
}
