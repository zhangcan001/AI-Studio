import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { NoWorkflowGuide } from "./NoWorkflowGuide";

describe("无工作流引导", () => {
  it("explains the local preparation path and exposes the two required actions", () => {
    const html = renderToStaticMarkup(
      <NoWorkflowGuide
        refreshing={false}
        onOpenWorkflows={vi.fn()}
        onReconnectComfy={vi.fn()}
        onRefresh={vi.fn()}
      />,
    );

    expect(html).toContain("还没有可用于创作的工作流");
    expect(html).toContain("启动 ComfyUI");
    expect(html).toContain("前往工作流管理");
    expect(html).toContain("测试 ComfyUI 连接");
    expect(html).not.toContain("workflow_library");
  });
});
