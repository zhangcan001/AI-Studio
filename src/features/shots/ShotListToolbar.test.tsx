import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { ShotListToolbar } from "./ShotListToolbar";
import { defaultShotListControls } from "./shotListQuery";

describe("ShotListToolbar", () => {
  it("renders search, status, page-size controls, and counts", () => {
    const html = renderToStaticMarkup(
      <ShotListToolbar
        controls={defaultShotListControls()}
        filteredCount={73}
        totalCount={500}
        pageStart={1}
        pageEnd={50}
        pageCount={2}
        onQueryChange={() => undefined}
        onStatusChange={() => undefined}
        sceneOptions={[{ value: "ALL", label: "全部镜头" }, { value: "UNASSIGNED", label: "未归档" }]}
        onSceneChange={() => undefined}
        onPageSizeChange={() => undefined}
        onPageChange={() => undefined}
      />,
    );

    expect(html).toContain("搜索镜头名称或提示词");
    expect(html).toContain("全部");
    expect(html).toContain("25 / 页");
    expect(html).toContain("50 / 页");
    expect(html).toContain("100 / 页");
    expect(html).toContain("结构筛选");
    expect(html).toContain('aria-label="镜头筛选"');
    expect(html).toContain('id="shot-list-filter-popover"');
    expect(html).toContain('hidden=""');
    expect(html).toContain("显示 1-50 / 匹配 73 / 总计 500");
  });
});
