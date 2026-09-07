// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ShotProductionProgress } from "./ShotProductionProgress";
import type { ShotProductionReadModel } from "./shotProductionState";

afterEach(() => {
  cleanup();
});

const model: ShotProductionReadModel = {
  steps: [
    { id: "prompt", label: "提示词", status: "COMPLETE", actionable: true },
    { id: "references", label: "参考", status: "COMPLETE", actionable: true },
    { id: "image", label: "图片", status: "BLOCKED", detail: "需要先填写提示词", actionable: true },
    { id: "image-review", label: "图片确认", status: "PENDING", actionable: true },
    { id: "video", label: "视频", status: "PENDING", actionable: true },
    { id: "video-review", label: "视频审核", status: "PENDING", actionable: true },
    { id: "complete", label: "完成", status: "PENDING", actionable: false },
  ],
  nextAction: { stepId: "image", label: "生成关键帧", detail: "需要先填写提示词", actionable: true },
};

describe("ShotProductionProgress", () => {
  it("exposes the pipeline as navigation-only controls", async () => {
    const user = userEvent.setup();
    const onNavigate = vi.fn();
    render(<ShotProductionProgress model={model} onNavigate={onNavigate} />);

    expect(screen.getByRole("navigation", { name: "镜头生产步骤" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "图片：受阻" }).getAttribute("aria-current")).toBe("step");
    expect(screen.getByText("需要先填写提示词")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "图片：受阻" }));
    await user.click(screen.getByRole("button", { name: "继续" }));
    expect(onNavigate).toHaveBeenNthCalledWith(1, "image");
    expect(onNavigate).toHaveBeenNthCalledWith(2, "image");
  });
});
