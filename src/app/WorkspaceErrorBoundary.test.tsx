import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { WorkspaceErrorFallback } from "./WorkspaceErrorBoundary";

describe("工作区错误隔离", () => {
  it("keeps recovery actions local to the failed video workspace", () => {
    const html = renderToStaticMarkup(
      <WorkspaceErrorFallback onBackToAssets={vi.fn()} onRetry={vi.fn()} />,
    );

    expect(html).toContain("当前工作区发生异常。");
    expect(html).toContain("返回素材库");
    expect(html).toContain("重新加载当前工作区");
    expect(html).toContain('role="alert"');
  });
});
