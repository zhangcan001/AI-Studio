import { describe, expect, it } from "vitest";
import { createDefaultOutputDraft } from "./WorkflowWorkspace";

describe("工作流输出映射默认值", () => {
  it("使用中文显示名称，同时保留技术输出 ID", () => {
    const draft = createDefaultOutputDraft();

    expect(draft.outputId).toBe("output_1");
    expect(draft.label).toBe("输出结果");
    expect(draft.label).not.toBe("Output");
  });
});
