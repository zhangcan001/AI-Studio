import { describe, expect, it } from "vitest";
import { workflowImportErrorView } from "./workflowSmartImportModel";

describe("workflowImportErrorView", () => {
  it("keeps a non-API workflow distinct from an unknown format", () => {
    expect(workflowImportErrorView(new Error("WORKFLOW_NOT_API_FORMAT"))).toMatchObject({
      kind: "NOT_API_FORMAT",
      message: "该文件不是 ComfyUI API 格式工作流，请重新导出 API 格式工作流。",
    });
    expect(workflowImportErrorView(new Error("UNKNOWN_FORMAT"))).toMatchObject({
      kind: "UNKNOWN_FORMAT",
      message: "这个 JSON 不是可识别的 ComfyUI 工作流。",
    });
  });
});
