import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { SettingsWorkspace } from "./SettingsWorkspace";

describe("设置与诊断界面", () => {
  it("keeps diagnostics actions and private path boundaries out of the UI", () => {
    const html = renderToStaticMarkup(
      <SettingsWorkspace
        connectionLoading={false}
        capabilityLoading={false}
        onReconnect={vi.fn()}
        onRefreshCapabilities={vi.fn()}
      />,
    );

    expect(html).toContain("设置与诊断");
    expect(html).toMatch(/刷新诊断|正在刷新/);
    expect(html).toContain("导出诊断包");
    expect(html).toContain("释放显存/内存");
    expect(html).toContain("模型文件不会删除");
    expect(html).toContain("接口地址");
    expect(html).not.toContain("AppData");
    expect(html).not.toContain("app.db");
    expect(html).not.toContain("工作流原文");
  });
});
