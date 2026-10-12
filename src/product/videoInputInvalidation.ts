// Notifications only. No domain state, snapshots, or competing input authority.
const listeners = new Set<(projectId: string) => void>();

/** Notify Create + Inspector to re-fetch formal ShotVideoInput via inputsGet. */
export function invalidateVideoInputs(projectId: string) {
  for (const listener of listeners) listener(projectId);
}

export function subscribeVideoInputInvalidation(listener: (projectId: string) => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Test helper: drop all listeners so vitest files do not cross-talk. */
export function resetVideoInputInvalidationForTests() {
  listeners.clear();
}
