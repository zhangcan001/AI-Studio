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
        onPageSizeChange={() => undefined}
        onPageChange={() => undefined}
      />,
    );

    expect(html).toContain("搜索镜头名称或 Prompt");
    expect(html).toContain("全部");
    expect(html).toContain("25 / 页");
    expect(html).toContain("50 / 页");
    expect(html).toContain("100 / 页");
    expect(html).toContain("显示 1-50 / 匹配 73 / 总计 500");
  });
});
