import { useEffect, useState } from "react";
import { api } from "../api";

export type HwWarning = {
  page: string;
  id: string;
  severity: "info" | "warning" | "danger";
  title: string;
  message: string;
};

export type HwProfile = {
  cpu: {
    name: string;
    cores: number;
    threads: number;
    maxMhz: number;
    tier: "budget" | "mid" | "high";
  };
  gpu: {
    name: string;
    vramMb: number;
    driver: string;
    tier: "integrated" | "budget" | "mid" | "high";
    isIntegrated: boolean;
    isOlderArch: boolean;
  };
  ram: {
    totalMb: number;
    speedMhz: number;
    sticks: number;
    tier: "low" | "ok" | "good";
  };
  storage: {
    hasNvme: boolean;
    hasSsd: boolean;
    hasHdd: boolean;
    tier: "nvme" | "sata_ssd" | "hdd" | "unknown";
  };
  isLaptop: boolean;
  isWifi: boolean;
  warnings: HwWarning[];
  tweakRisks: Record<string, TweakRisk>;
};

export type TweakRisk = {
  severity: "info" | "warning" | "danger";
  title: string;
  message: string;
};

// module-level cache — survives re-renders, cleared on app restart
let _cached: HwProfile | null = null;
let _loading = false;
const _listeners = new Set<(p: HwProfile) => void>();

export function useHwProfile(): HwProfile | null {
  const [profile, setProfile] = useState<HwProfile | null>(_cached);

  useEffect(() => {
    if (_cached) { setProfile(_cached); return; }

    const update = (p: HwProfile) => setProfile(p);
    _listeners.add(update);

    if (!_loading) {
      _loading = true;
      api.hwProfile()
        .then((p: any) => {
          _cached = p as HwProfile;
          _listeners.forEach(fn => fn(_cached!));
        })
        .catch(() => { _loading = false; })
        .finally(() => { _loading = false; });
    }

    return () => { _listeners.delete(update); };
  }, []);

  return profile;
}

export function useHwWarnings(page: string): HwWarning[] {
  const profile = useHwProfile();
  if (!profile) return [];
  return profile.warnings.filter(w => w.page === page);
}

// Hardware-aware risk verdict for one specific tweak id, or undefined if this
// tweak has no known hardware-specific risk on the detected system.
export function useTweakRisk(id: string): TweakRisk | undefined {
  const profile = useHwProfile();
  return profile?.tweakRisks?.[id];
}
