import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { WorkspaceErrorFallback } from "./WorkspaceErrorBoundary";

describe("工作区错误隔离", () => {
  it("keeps recovery actions local to the failed video workspace", () => {
    const html = renderToStaticMarkup(
      <WorkspaceErrorFallback onBackToAssets={vi.fn()} onRetry={vi.fn()} />,
    );

    expect(html).toContain("批量视频页面发生异常。");
    expect(html).toContain("返回资产库");
    expect(html).toContain("重新打开批量视频");
    expect(html).toContain('role="alert"');
  });
});
