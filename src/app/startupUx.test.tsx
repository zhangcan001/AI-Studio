import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { StartupScreen } from "./StartupScreen";

describe("启动引导界面", () => {
  it("shows a truthful preparation message without a fake progress value", () => {
    const html = renderToStaticMarkup(<StartupScreen onRetry={vi.fn()} />);
    expect(html).toContain("正在准备创作环境");
    expect(html).not.toMatch(/\d+%/);
  });

  it("offers a retry when bootstrap fails", () => {
    const html = renderToStaticMarkup(<StartupScreen error="应用初始化失败，请稍后重试。" onRetry={vi.fn()} />);
    expect(html).toContain("创作环境准备失败");
    expect(html).toContain("重试");
  });
});
