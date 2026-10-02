// Notifications only. No domain state, snapshots, or competing event authority.
const listeners = new Set<(projectId: string) => void>();
export function invalidateRuns(projectId: string) { for (const listener of listeners) listener(projectId); }
export function subscribeRunInvalidation(listener: (projectId: string) => void) {
  listeners.add(listener); return () => { listeners.delete(listener); };
}
