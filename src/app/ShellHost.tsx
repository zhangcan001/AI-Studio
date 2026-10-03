import { AppShellV3, type AppShellV3Props } from "./v3/AppShellV3";

/** V3 is the only runtime shell. Deprecated local preferences are not read. */
export function ShellHost(props: AppShellV3Props) {
  return <AppShellV3 {...props} />;
}
