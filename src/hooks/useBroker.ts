import { useEffect, useState } from "react";
import { api } from "../api";
import { rescanHealth } from "./useHealthScan";

/** Admin helper (broker) state, shared by every component. `available` is
 * false unless the app was built with the `broker` feature. */
export type BrokerState = { available: boolean; connected: boolean };

let state: BrokerState = { available: false, connected: false };
const listeners = new Set<(s: BrokerState) => void>();
const publish = (s: BrokerState) => { state = s; listeners.forEach((l) => l(s)); };

export const refreshBroker = () =>
  api.brokerStatus().then((s) => { publish(s); return s; }).catch(() => state);

/** Starts the helper (Windows asks via UAC) and refreshes the shared state. */
export async function unlockBroker(): Promise<void> {
  try {
    await api.brokerUnlock();
  } finally {
    await refreshBroker();
  }
  // The health check marks admin-only items as blocked; rescan so they unlock.
  await rescanHealth().catch(() => {});
}

export function useBroker(): BrokerState {
  const [s, setS] = useState(state);
  useEffect(() => {
    listeners.add(setS);
    refreshBroker();
    return () => { listeners.delete(setS); };
  }, []);
  return s;
}
