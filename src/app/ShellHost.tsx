import { StudioShell, type StudioShellProps } from "./StudioShell";
import { AppShellV3 } from "./v3/AppShellV3";
import type { AppRoute } from "./routes/types";

export const SHELL_MODE_KEY = "aistudio.shellMode";
/** Native migration gate passed; an explicit preference retains the old shell. */
export function readShellMode(): "v3" | "legacy" {
  try { return localStorage.getItem(SHELL_MODE_KEY) === "legacy" ? "legacy" : "v3"; } catch { return "v3"; }
}
export function ShellHost({ mode, route, navigate, back, onModeChange, ...legacy }: StudioShellProps & { mode: "v3" | "legacy"; route: AppRoute; navigate: (route: AppRoute) => void; back: () => void; onModeChange: (mode: "v3" | "legacy") => void }) {
  if (mode === "legacy") return <StudioShell {...legacy}><button type="button" className="quiet-button" onClick={() => onModeChange("v3")}>试用项目导航</button>{legacy.children}</StudioShell>;
  return <AppShellV3 route={route} projectName={legacy.project?.name} projectSelector={legacy.projectSelector} navigate={navigate} back={back}><button type="button" className="quiet-button" onClick={() => onModeChange("legacy")}>返回旧版导航</button>{legacy.children}</AppShellV3>;
}
