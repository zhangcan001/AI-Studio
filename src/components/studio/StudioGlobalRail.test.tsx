import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { defaultStudioRailItems, StudioGlobalRail } from "./StudioGlobalRail";

describe("StudioGlobalRail", () => {
  it("includes workflows in the default menu between review and analysis", () => {
    expect(defaultStudioRailItems.map((item) => item.id)).toEqual([
      "project",
      "creation",
      "assets",
      "production",
      "review",
      "workflows",
      "analysis",
      "settings",
    ]);
    expect(defaultStudioRailItems[5]).toMatchObject({
      id: "workflows",
      label: "工作流",
      destination: "workflows",
      icon: "workflows",
      hint: "添加和管理 ComfyUI 工作流",
    });
    const html = renderToStaticMarkup(<StudioGlobalRail onNavigate={vi.fn()} />);
    expect(html).toContain('aria-label="工作流：添加和管理 ComfyUI 工作流"');
    expect(html).toContain('title="添加和管理 ComfyUI 工作流"');
  });

  it.each(["creation", "production", "review", "workflows"] as const)("marks only %s as active", (section) => {
    const html = renderToStaticMarkup(<StudioGlobalRail activeItem={section} onNavigate={vi.fn()} />);
    const item = defaultStudioRailItems.find(({ id }) => id === section);

    expect(html.match(/studio-global-rail__item--active/g)).toHaveLength(1);
    expect(html).toContain('aria-current="page"');
    expect(html).toContain(`title="${item?.hint}"`);
  });
});
