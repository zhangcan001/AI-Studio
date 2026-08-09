import { describe, expect, it } from "vitest";
import { generationBlockedReason } from "./generationBlockedReason";

const ready = {
  productionBusy: false,
  comfyConnected: true,
  taskEventsReady: true,
  missingAsset: false,
  validationError: false,
  unsupportedField: false,
};

describe("生成操作区阻塞原因", () => {
  it("按生产队列、运行环境、事件通道和输入顺序只返回一条原因", () => {
    expect(generationBlockedReason({ ...ready, productionBusy: true, comfyConnected: false })).toBe(
      "生产队列正在运行，请先暂停或等待完成。",
    );
    expect(generationBlockedReason({ ...ready, comfyConnected: false })).toBe(
      "ComfyUI 当前离线，请连接后再开始生成。",
    );
    expect(generationBlockedReason({ ...ready, taskEventsReady: false, taskEventError: "任务事件通道不可用" })).toBe(
      "任务事件通道不可用",
    );
    expect(generationBlockedReason({ ...ready, missingAsset: true, validationError: true })).toBe(
      "请先选择所需素材。",
    );
  });

  it("在所有条件满足时不阻塞生成", () => {
    expect(generationBlockedReason(ready)).toBeUndefined();
  });
});
