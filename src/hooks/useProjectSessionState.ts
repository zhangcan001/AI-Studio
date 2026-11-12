import { useEffect, useState } from "react";

/** UI navigation only. Never persist media bindings, generation values or drafts here. */
export function useProjectSessionState<T>(projectId: string, name: string, initial: T, valid: (value: unknown) => value is T) {
  const key = JSON.stringify(["ai-studio.navigation.v1", projectId, name]);
  const read = () => {
    try { const value: unknown = JSON.parse(sessionStorage.getItem(key) ?? "null"); if (valid(value)) return value; } catch { /* unavailable or malformed storage */ }
    return initial;
  };
  const [state, setState] = useState(() => ({ key, value: read() }));
  const value = state.key === key ? state.value : read();
  useEffect(() => { setState(current => current.key === key ? current : { key, value: read() }); }, [key]);
  const setValue = (next: T) => {
    setState({ key, value: next });
    try { sessionStorage.setItem(key, JSON.stringify(next)); } catch { /* keep working without storage */ }
  };
  return [value, setValue] as const;
}
