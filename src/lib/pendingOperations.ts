let pending = 0;
const listeners = new Set<() => void>();
export const pendingOperations = {
  getSnapshot: () => pending,
  subscribe: (listener: () => void) => {
    listeners.add(listener);
    return () => { listeners.delete(listener); };
  },
};

export function beginOperation() {
  pending++;
  listeners.forEach(listener => listener());
  let ended = false;
  return () => {
    if (ended) return;
    ended = true;
    pending--;
    listeners.forEach(listener => listener());
  };
}

export async function trackOperation<T>(run: () => Promise<T>): Promise<T> {
  const end = beginOperation();
  try { return await run(); }
  finally { end(); }
}
