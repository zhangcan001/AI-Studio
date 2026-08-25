import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { StudioShell } from "./StudioShell";
import { shotWorkspaceModeForSection, studioRouteForSection } from "./studioNavigation";

describe("StudioShell C workspace navigation", () => {
  it.each([
    ["creation", "creation"],
    ["production", "production"],
    ["review", "review"],
  ] as const)("routes %s to the shots workspace and highlights the matching rail item", (section, mode) => {
    const route = studioRouteForSection(section);
    const html = renderToStaticMarkup(
      <StudioShell
        workspace={route.workspace}
        currentSection={route.section}
        onNavigate={vi.fn()}
      >
        <div data-studio-mode={shotWorkspaceModeForSection(section)}>C Shot workspace</div>
      </StudioShell>,
    );

    expect(route.workspace).toBe("shots");
    expect(html).toContain(`data-studio-mode="${mode}"`);
    expect(html.match(/studio-global-rail__item--active/g)).toHaveLength(1);
    expect(html).toContain(`aria-label="${section === "creation" ? "创作：镜头创作工作区" : section === "production" ? "生产：生产队列与批量运行" : "审核：镜头审核与任务"}"`);
  });
});
