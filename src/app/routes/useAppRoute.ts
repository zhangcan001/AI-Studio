import { useCallback, useEffect, useReducer, useState } from "react";
import { appRouteReducer, initialRouteState } from "./reducer";
import { writeRouteResume } from "./resumeAdapter";
import type { AppRoute } from "./types";

/** Sole runtime location owner. Persistence is an output, never a second owner. */
export function useAppRoute() {
  const [state, dispatch] = useReducer(appRouteReducer, initialRouteState);
  const [ready, setReady] = useState(false);
  const navigate = useCallback((route: AppRoute) => dispatch({ type: "navigate", route }), []);
  const restore = useCallback((route: AppRoute) => { dispatch({ type: "restore", route }); setReady(true); }, []);
  const back = useCallback(() => dispatch({ type: "back" }), []);
  const switchProject = useCallback((projectId: string) => dispatch({ type: "switch-project", projectId }), []);
  useEffect(() => { if (ready) writeRouteResume(state.current); }, [ready, state.current]);
  return { route: state.current, navigate, restore, back, switchProject };
}
