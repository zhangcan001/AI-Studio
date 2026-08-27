// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { AssetView } from "../../types/asset";
import type { ReviewCompareCandidate, ReviewCompareItem } from "../../types/reviewProductivity";
import { ReviewCompareWorkspace } from "./ReviewCompareWorkspace";

const asset = (id: string, kind: "image" | "video" = "image"): AssetView => ({
  id,
  assetType: kind,
  category: kind === "video" ? "generated_video" : "generated_image",
  name: id,
  originalName: id,
  mimeType: kind === "video" ? "video/mp4" : "image/png",
  fileSize: 12,
  createdAt: "2026-08-27T00:00:00Z",
  isFavorite: false,
  tags: [],
});

const candidate = (id: string, overrides: Partial<ReviewCompareCandidate> = {}): ReviewCompareCandidate => ({
  id,
  asset: asset(id),
  imageUrl: `blob:${id}`,
  ...overrides,
});

const context = {
  source: "snapshot" as const,
  historicalName: "历史镜头名",
  currentName: "当前镜头名",
  prompt: "雨夜巷口",
  context: "scene context",
  workflowName: "H3 Workflow",
  recipeName: "I2V Recipe",
  contextHash: "sha256:abc",
  referenceSets: [{ id: "set-1", name: "主角参考集", assets: [] }],
  referenceAssets: [{ id: "ref-1", name: "主角正面" }],
  outputSpec: { width: 1280, height: 720 },
  stageInput: "关键帧 A",
  readiness: { status: "READY" },
};

const item = (overrides: Partial<ReviewCompareItem> = {}): ReviewCompareItem => ({
  id: "shot-1",
  ordinal: 0,
  shotId: "shot-1",
  shotName: "Shot 01",
  candidates: [candidate("a", { label: "候选 A", context }), candidate("b", { label: "候选 B", context })],
  reviewNote: "",
  ...overrides,
});

const panel = (slot: "A" | "B") => screen.getByRole("article", { name: `${slot} 槽位` });
const workspace = () => screen.getByRole("region", { name: "候选结果对比工作区" });
const slotButton = (title: string, slot: "A" | "B") => {
  const button = screen.getAllByRole("button", { name: `将 ${title} 放入 ${slot} 槽位` }).find((element) => element.textContent === slot);
  if (!button) throw new Error(`Missing ${slot} slot button for ${title}`);
  return button;
};

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("ReviewCompareWorkspace", () => {
  it("keeps a single candidate local to the A slot and does not invoke mutation callbacks", async () => {
    const user = userEvent.setup();
    const onItemChange = vi.fn();
    const onApprove = vi.fn();
    const onReject = vi.fn();

    render(
      <ReviewCompareWorkspace
        items={[item({ candidates: [candidate("only", { label: "唯一候选" })] })]}
        onItemChange={onItemChange}
        onApprove={onApprove}
        onReject={onReject}
      />,
    );

    expect(panel("A").textContent).toContain("唯一候选");
    expect(screen.queryByRole("article", { name: "B 槽位" })).toBeNull();
    expect(screen.queryByRole("button", { name: "交换 A/B 槽位" })).toBeNull();

    await user.click(screen.getByRole("button", { name: "将 唯一候选 放入 A 槽位" }));

    expect(panel("A").textContent).toContain("唯一候选");
    expect(onItemChange).not.toHaveBeenCalled();
    expect(onApprove).not.toHaveBeenCalled();
    expect(onReject).not.toHaveBeenCalled();
  });

  it("moves candidates between A/B by their slot buttons and swaps them locally", async () => {
    const user = userEvent.setup();
    const onItemChange = vi.fn();
    const onApprove = vi.fn();
    const onReject = vi.fn();

    render(<ReviewCompareWorkspace items={[item()]} onItemChange={onItemChange} onApprove={onApprove} onReject={onReject} />);

    await user.click(screen.getByRole("button", { name: "将 候选 A 放入 B 槽位" }));
    await user.click(screen.getByRole("button", { name: "将 候选 B 放入 A 槽位" }));

    expect(panel("A").textContent).toContain("候选 B");
    expect(panel("B").textContent).toContain("候选 A");

    await user.click(screen.getByRole("button", { name: "交换 A/B 槽位" }));

    expect(panel("A").textContent).toContain("候选 A");
    expect(panel("B").textContent).toContain("候选 B");
    expect(onItemChange).not.toHaveBeenCalled();
    expect(onApprove).not.toHaveBeenCalled();
    expect(onReject).not.toHaveBeenCalled();
  });

  it("navigates with ArrowRight and ArrowLeft and reports the changed item", async () => {
    const first = item();
    const second = item({ id: "shot-2", ordinal: 1, shotName: "Shot 02" });
    const onItemChange = vi.fn();

    render(<ReviewCompareWorkspace items={[first, second]} onItemChange={onItemChange} />);

    await act(async () => {
      fireEvent.keyDown(workspace(), { key: "ArrowRight" });
    });
    await waitFor(() => expect(screen.getByRole("heading", { level: 2, name: "Shot 02" })).toBeTruthy());
    expect(onItemChange).toHaveBeenLastCalledWith(second);

    await act(async () => {
      fireEvent.keyDown(workspace(), { key: "ArrowLeft" });
    });
    await waitFor(() => expect(screen.getByRole("heading", { level: 2, name: "Shot 01" })).toBeTruthy());
    expect(onItemChange).toHaveBeenLastCalledWith(first);
  });

  it("focuses the public A/B panels with the 1 and 2 keyboard shortcuts", async () => {
    render(<ReviewCompareWorkspace items={[item()]} />);
    const root = workspace();
    root.focus();

    await act(async () => {
      fireEvent.keyDown(root, { key: "2" });
    });
    expect(document.activeElement).toBe(panel("B"));

    await act(async () => {
      fireEvent.keyDown(root, { key: "1" });
    });
    expect(document.activeElement).toBe(panel("A"));
  });

  it("does not mutate or trigger an action for Enter or Space on the workspace", async () => {
    const callbacks = {
      onItemChange: vi.fn(),
      onConfirmAndApprove: vi.fn(),
      onApprove: vi.fn(),
      onStar: vi.fn(),
      onReject: vi.fn(),
      onRegenerate: vi.fn(),
      onCreateReworkBatch: vi.fn(),
    };

    render(<ReviewCompareWorkspace items={[item()]} {...callbacks} />);
    const root = workspace();
    root.focus();

    await act(async () => {
      fireEvent.keyDown(root, { key: "Enter" });
      fireEvent.keyDown(root, { key: " " });
    });

    for (const callback of Object.values(callbacks)) expect(callback).not.toHaveBeenCalled();
    expect(panel("A").textContent).toContain("候选 A");
    expect(panel("B").textContent).toContain("候选 B");
  });

  it("requires an explicit confirm-and-approve click and does not auto-advance", async () => {
    const user = userEvent.setup();
    const first = item();
    const second = item({ id: "shot-2", ordinal: 1, shotName: "Shot 02" });
    const onConfirmAndApprove = vi.fn();
    const onItemChange = vi.fn();

    render(<ReviewCompareWorkspace items={[first, second]} onConfirmAndApprove={onConfirmAndApprove} onItemChange={onItemChange} />);

    const confirmButton = screen.getByRole("button", { name: "确认并通过" });
    expect(onConfirmAndApprove).not.toHaveBeenCalled();

    await user.click(confirmButton);

    await waitFor(() => expect(onConfirmAndApprove).toHaveBeenCalledWith(first.candidates[0], first));
    expect(screen.getByRole("heading", { level: 2, name: "Shot 01" })).toBeTruthy();
    expect(onItemChange).not.toHaveBeenCalled();
  });

  it("renders all explicitly supplied action controls", () => {
    render(
      <ReviewCompareWorkspace
        items={[item()]}
        onConfirmAndApprove={vi.fn()}
        onApprove={vi.fn()}
        onStar={vi.fn()}
        onReject={vi.fn()}
        onRegenerate={vi.fn()}
        onCreateReworkBatch={vi.fn()}
        onSaveNote={vi.fn()}
      />,
    );

    for (const label of ["确认并通过", "仅通过", "标星", "拒绝", "标记返工", "创建返工批次", "保存备注"]) {
      expect(screen.getByRole("button", { name: label })).toBeTruthy();
    }
  });

  it("confirms or cancels navigation when a note is dirty", async () => {
    const user = userEvent.setup();
    const first = item();
    const second = item({ id: "shot-2", ordinal: 1, shotName: "Shot 02" });
    const onBeforeItemChange = vi.fn().mockResolvedValueOnce(false).mockResolvedValueOnce(true);
    const onItemChange = vi.fn();

    render(<ReviewCompareWorkspace items={[first, second]} onBeforeItemChange={onBeforeItemChange} onItemChange={onItemChange} />);

    await user.type(screen.getByRole("textbox", { name: "审核备注" }), "需要返工");
    expect(screen.getByText("备注未保存；切换审核项时会先提示。")).toBeTruthy();

    const nextButton = screen.getByRole("button", { name: "下一项" });
    await user.click(nextButton);
    await waitFor(() => expect(onBeforeItemChange).toHaveBeenCalledWith(first, second, "需要返工"));
    expect(screen.getByRole("heading", { level: 2, name: "Shot 01" })).toBeTruthy();
    expect(onItemChange).not.toHaveBeenCalled();

    await user.click(nextButton);
    await waitFor(() => expect(screen.getByRole("heading", { level: 2, name: "Shot 02" })).toBeTruthy());
    expect(onItemChange).toHaveBeenCalledWith(second);
  });

  it("uses the browser confirmation fallback for a dirty note", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    const first = item();
    const second = item({ id: "shot-2", ordinal: 1, shotName: "Shot 02" });
    const onItemChange = vi.fn();

    render(<ReviewCompareWorkspace items={[first, second]} onItemChange={onItemChange} />);
    await user.type(screen.getByRole("textbox", { name: "审核备注" }), "待确认");
    await user.click(screen.getByRole("button", { name: "下一项" }));

    await waitFor(() => expect(confirm).toHaveBeenCalledWith("当前备注尚未保存，确定切换审核项吗？"));
    expect(screen.getByRole("heading", { level: 2, name: "Shot 01" })).toBeTruthy();
    expect(onItemChange).not.toHaveBeenCalled();

    confirm.mockReturnValue(true);
    await user.click(screen.getByRole("button", { name: "下一项" }));
    await waitFor(() => expect(screen.getByRole("heading", { level: 2, name: "Shot 02" })).toBeTruthy());
    expect(onItemChange).toHaveBeenCalledWith(second);
  });

  it("enforces the 4 KiB UTF-8 note limit in the public form", async () => {
    const onSaveNote = vi.fn();
    render(<ReviewCompareWorkspace items={[item()]} onSaveNote={onSaveNote} />);

    const note = screen.getByRole("textbox", { name: "审核备注" }) as HTMLTextAreaElement;
    const saveButton = screen.getByRole("button", { name: "保存备注" }) as HTMLButtonElement;
    const exactLimit = `${"界".repeat(1365)}a`;
    const overLimit = "界".repeat(1366);

    await act(async () => {
      fireEvent.change(note, { target: { value: exactLimit } });
    });
    expect(screen.getByText("4096 / 4096 bytes")).toBeTruthy();
    expect(screen.queryByText("备注不能超过 4 KiB。")).toBeNull();
    expect(saveButton.disabled).toBe(false);

    await act(async () => {
      fireEvent.change(note, { target: { value: overLimit } });
    });
    expect(screen.getByText("4098 / 4096 bytes")).toBeTruthy();
    expect(screen.getByText("备注不能超过 4 KiB。")).toBeTruthy();
    expect(saveButton.disabled).toBe(true);
  });

  it("renders both video candidates as native metadata-preloaded controls", () => {
    render(
      <ReviewCompareWorkspace
        items={[
          item({
            candidates: [
              candidate("video-a", { mediaKind: "video", mediaUrl: "/media/a.mp4", label: "视频 A" }),
              candidate("video-b", { mediaKind: "video", mediaUrl: "/media/b.mp4", label: "视频 B" }),
            ],
          }),
        ]}
      />,
    );

    const videos = [screen.getByLabelText("A 视频 A"), screen.getByLabelText("B 视频 B")];
    expect(videos).toHaveLength(2);
    for (const video of videos) {
      const element = video as HTMLVideoElement;
      expect(element.tagName).toBe("VIDEO");
      expect(element.controls).toBe(true);
      expect(element.preload).toBe("metadata");
      expect(element.playsInline).toBe(true);
    }
  });

  it("keeps a partial failure manual and does not auto-advance after the successful candidate is explicitly approved", async () => {
    const user = userEvent.setup();
    const first = item({
      candidates: [
        candidate("failed", { label: "失败候选", productionItemStatus: "FAILED" }),
        candidate("success", { label: "成功候选", productionItemStatus: "SUCCEEDED" }),
      ],
    });
    const second = item({ id: "shot-2", ordinal: 1, shotName: "Shot 02" });
    const onConfirmAndApprove = vi.fn();
    const onItemChange = vi.fn();

    render(<ReviewCompareWorkspace items={[first, second]} onConfirmAndApprove={onConfirmAndApprove} onItemChange={onItemChange} />);

    expect(screen.queryByRole("button", { name: "确认并通过" })).toBeNull();
    expect((screen.getByRole("button", { name: "仅通过" }) as HTMLButtonElement).disabled).toBe(true);

    await user.click(slotButton("成功候选", "A"));
    expect(screen.getByRole("button", { name: "确认并通过" })).toBeTruthy();
    expect(onConfirmAndApprove).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "确认并通过" }));
    await waitFor(() => expect(onConfirmAndApprove).toHaveBeenCalledWith(first.candidates[1], first));
    expect(screen.getByRole("heading", { level: 2, name: "Shot 01" })).toBeTruthy();
    expect(onItemChange).not.toHaveBeenCalled();
  });

  it("renders the historical context and legacy fallback through the visible inspector", () => {
    render(<ReviewCompareWorkspace items={[item()]} />);

    expect(screen.getByText("历史快照")).toBeTruthy();
    expect(screen.getByText("历史镜头名")).toBeTruthy();
    expect(screen.getByText("雨夜巷口")).toBeTruthy();
    expect(screen.getByText("sha256:abc")).toBeTruthy();
    expect(screen.getByText("主角参考集")).toBeTruthy();
    expect(screen.getByText("主角正面")).toBeTruthy();
    expect(screen.getByText("当前 Workflow 未提供独立 Negative Prompt 输入")).toBeTruthy();

    cleanup();
    render(
      <ReviewCompareWorkspace
        items={[
          item({
            name: "当前名称",
            shotName: undefined,
            context: undefined,
            historicalContext: undefined,
            contextSnapshot: undefined,
            snapshot: undefined,
            candidates: [candidate("only", { context: undefined })],
          }),
        ]}
      />,
    );

    expect(screen.getByText("旧版任务，无生产准备快照")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 2, name: "当前名称" })).toBeTruthy();
    expect(screen.queryByText("历史名称")).toBeNull();
  });

  it("does not expose confirm-and-approve for a missing Shot link", () => {
    render(
      <ReviewCompareWorkspace
        items={[item({ shotId: undefined })]}
        onConfirmAndApprove={vi.fn()}
        onApprove={vi.fn()}
        onStar={vi.fn()}
        onReject={vi.fn()}
        onRegenerate={vi.fn()}
      />,
    );

    expect(screen.queryByRole("button", { name: "确认并通过" })).toBeNull();
    expect(screen.getByRole("button", { name: "仅通过" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "标星" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "拒绝" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "标记返工" })).toBeTruthy();
  });
});
