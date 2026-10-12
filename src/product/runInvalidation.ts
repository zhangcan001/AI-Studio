// Notifications only. No domain state, snapshots, or competing event authority.
export type RunInvalidationOptions = {
  // Default true: Library + Runs. Set false for generate/start so Runs refresh without Library thrash.
  library?: boolean;
};
type Listener = (projectId: string, options?: RunInvalidationOptions) => void;
const listeners = new Set<Listener>();
export function invalidateRuns(projectId: string, options?: RunInvalidationOptions) {
  for (const listener of listeners) listener(projectId, options);
}
export function subscribeRunInvalidation(listener: Listener) {
  listeners.add(listener); return () => { listeners.delete(listener); };
}
