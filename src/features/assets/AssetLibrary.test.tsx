import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { AssetLibrary } from "./AssetLibrary";

describe("资产库本地导入入口", () => {
  it("shows the native multi-file import action without replacing the library", () => {
    const html = renderToStaticMarkup(
      <AssetLibrary
        projectId="project-1"
        onUseInStudio={vi.fn()}
        onOpenVideoBatch={vi.fn()}
        onOpenTask={vi.fn()}
      />,
    );

    expect(html).toContain("导入本地素材");
    expect(html).toContain("资产库");
    expect(html).toContain("搜索、筛选、对比当前项目的源素材和生成结果");
    expect(html).not.toContain('type="file"');
  });
});
