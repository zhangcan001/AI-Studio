import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { StudioGlobalRail } from "./StudioGlobalRail";

describe("StudioGlobalRail", () => {
  it.each(["creation", "production", "review"] as const)("marks only %s as active", (section) => {
    const html = renderToStaticMarkup(<StudioGlobalRail activeItem={section} onNavigate={vi.fn()} />);

    expect(html.match(/studio-global-rail__item--active/g)).toHaveLength(1);
    expect(html).toContain('aria-current="page"');
    expect(html).toContain(`title="${section === "creation" ? "镜头创作工作区" : section === "production" ? "生产队列与批量运行" : "镜头审核与任务"}"`);
  });
});
