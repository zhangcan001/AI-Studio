import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { ShellHost } from "./ShellHost";
import { studioRouteForSection } from "./studioNavigation";
import { fromLegacyLocation } from "./routes/legacyAdapter";

describe("Canonical shell navigation preserves former StudioShell intent", () => {
  it.each(["creation", "production", "review"] as const)("routes old %s to its canonical V3 page and highlights one item", section => {
    const old = studioRouteForSection(section);
    const route = fromLegacyLocation({ ...old, projectId: "p" });
    const html = renderToStaticMarkup(<ShellHost route={route} projectSelector={null} navigate={vi.fn()} back={vi.fn()}><div>content</div></ShellHost>);
    expect(route.kind).toBe(section === "creation" ? "create" : "runs");
    expect(html.match(/aria-current="page"/g)).toHaveLength(1);
    expect(html).not.toContain("studio-global-rail");
    expect(html).toContain(section === "creation" ? "创作" : "运行");
  });
  it("keeps workflows under explicit Advanced navigation rather than a normal rail item", () => {
    const route=fromLegacyLocation({workspace:"workflows",projectId:"p"});
    const html=renderToStaticMarkup(<ShellHost route={route} projectSelector={null} navigate={vi.fn()} back={vi.fn()}><div>Workflow Lab</div></ShellHost>);
    expect(route).toMatchObject({kind:"project-settings",section:"advanced-workflows"});
    expect(html).toContain("高级工作流");expect(html).not.toMatch(/aria-current="page"[^>]*>创作/);expect(html).not.toContain("返回旧版导航");
  });
});
