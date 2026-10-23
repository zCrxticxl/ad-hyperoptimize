import { useEffect, useState } from "react";
import { api } from "../api";
import type { HealthScan } from "../lib/health";

// One scan shared by Health Check, Optimize My PC and Optimize: switching tabs
// reuses the last result instead of re-reading the whole system each time.
// Any page can force a rescan (after applying or undoing), and every mounted
// page receives the new result.
let cached: HealthScan | null = null;
let inFlight: Promise<HealthScan> | null = null;
const listeners = new Set<(s: HealthScan | null, busy: boolean, err: string) => void>();

function notify(busy: boolean, err = "") {
  listeners.forEach((l) => l(cached, busy, err));
}

export function rescanHealth(): Promise<HealthScan> {
  if (inFlight) return inFlight;
  notify(true);
  inFlight = api
    .healthScan()
    .then((s: HealthScan) => {
      if (s?.error) throw new Error(s.error);
      cached = s;
      notify(false);
      return s;
    })
    .catch((e: unknown) => {
      notify(false, String(e));
      throw e;
    })
    .finally(() => {
      inFlight = null;
    });
  return inFlight;
}

export function useHealthScan() {
  const [scan, setScan] = useState<HealthScan | null>(cached);
  const [busy, setBusy] = useState(inFlight !== null);
  const [error, setError] = useState("");

  useEffect(() => {
    const l = (s: HealthScan | null, b: boolean, e: string) => {
      setScan(s);
      setBusy(b);
      setError(e);
    };
    listeners.add(l);
    if (!cached && !inFlight) rescanHealth().catch(() => {});
    else if (inFlight) setBusy(true);
    return () => {
      listeners.delete(l);
    };
  }, []);

  const rescan = () => rescanHealth().catch(() => {});
  return { scan, busy, error, rescan };
}
