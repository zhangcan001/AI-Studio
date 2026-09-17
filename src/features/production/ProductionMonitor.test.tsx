// @vitest-environment jsdom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { openArtifact, revealArtifact } from "../../services/tauriClient";
import type { ArtifactDto, ProductionBatchArtifactsDto, ProductionTaskDto } from "../../types/artifact";
import { ProductionMonitor } from "./ProductionMonitor";

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return {
    ...actual,
    openArtifact: vi.fn(),
    revealArtifact: vi.fn(),
    getAssetMediaUrl: vi.fn((_projectId: string, assetId: string) => `asset://${assetId}`),
  };
});

afterEach(() => { cleanup(); vi.clearAllMocks(); });

function artifact(id: string, overrides: Partial<ArtifactDto> = {}): ArtifactDto {
  return {
    id,
    taskId: "task-1",
    outputId: "output-video",
    ordinal: 0,
    mediaType: "video",
    name: `${id}.mp4`,
    mimeType: "video/mp4",
    width: 1280,
    height: 720,
    durationMs: 5000,
    sizeBytes: 2048,
    version: 1,
    createdAt: "2026-09-17T00:00:00Z",
    thumbnailAvailable: false,
    availability: "available",
    reviewStatus: "PENDING",
    ...overrides,
  };
}

function task(ordinal: number, status: string, artifacts: ArtifactDto[] = [], overrides: Partial<ProductionTaskDto> = {}): ProductionTaskDto {
  return {
    productionItemId: `item-${ordinal}`,
    ordinal,
    productionItemStatus: status,
    task: {
      id: `task-${ordinal}`,
      status: status === "SUCCEEDED" ? "SUCCEEDED" : status,
      workflowVersionId: "wfv-test",
      recipeId: "rcp-test",
      createdAt: "2026-09-17T00:00:00Z",
    },
    artifacts,
    ...overrides,
  };
}

function batch(items: ProductionTaskDto[], status = "COMPLETED"): ProductionBatchArtifactsDto {
  return {
    batchId: "batch-test",
    batchName: "测试批次",
    status,
    total: items.length,
    pending: items.filter((item) => ["READY", "PENDING"].includes(item.productionItemStatus)).length,
    running: items.filter((item) => ["RUNNING", "DISPATCHING", "DISPATCHED"].includes(item.productionItemStatus)).length,
    succeeded: items.filter((item) => item.productionItemStatus === "SUCCEEDED").length,
    failed: items.filter((item) => item.productionItemStatus === "FAILED").length,
    cancelled: items.filter((item) => item.productionItemStatus === "CANCELLED").length,
    skipped: items.filter((item) => item.productionItemStatus === "SKIPPED").length,
    items,
  };
}

describe("ProductionMonitor artifact workflow", () => {
  it("shows artifacts for a completed task and opens by artifact ID", async () => {
    const user = userEvent.setup();
    render(<ProductionMonitor projectId="project-1" batch={batch([task(1, "SUCCEEDED", [artifact("asset-video")])])} />);

    expect(screen.queryByText("任务已完成，但没有登记可用产物。")).toBeNull();
    expect(screen.getByText("asset-video.mp4")).toBeTruthy();
    const artifactCard = screen.getByText("asset-video.mp4").closest("article");
    expect(artifactCard).toBeTruthy();
    expect(within(artifactCard as HTMLElement).getByText("asset-video")).toBeTruthy();
    expect(within(artifactCard as HTMLElement).getByText("可用")).toBeTruthy();
    expect(screen.getAllByText("已完成").length).toBeGreaterThan(0);
    const open = screen.getByRole("button", { name: "打开" });
    expect((open as HTMLButtonElement).disabled).toBe(false);
    await user.click(open);
    await waitFor(() => expect(openArtifact).toHaveBeenCalledWith("asset-video"));
  });

  it("disables actions and explains a missing artifact", () => {
    render(<ProductionMonitor projectId="project-1" batch={batch([task(1, "SUCCEEDED", [artifact("asset-missing", { availability: "missing" })])])} />);

    expect(screen.getByText("文件不存在")).toBeTruthy();
    expect(screen.getByText("文件不存在，无法预览或打开。")).toBeTruthy();
    const artifactCard = screen.getByText("asset-missing.mp4").closest("article");
    expect(artifactCard).toBeTruthy();
    expect(within(artifactCard as HTMLElement).getByText("asset-missing")).toBeTruthy();
    expect(within(artifactCard as HTMLElement).getByText("文件不存在")).toBeTruthy();
    expect((screen.getByRole("button", { name: "打开" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "在文件夹中显示" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("renders multiple outputs independently and keeps each action bound to its artifact", async () => {
    const user = userEvent.setup();
    render(<ProductionMonitor projectId="project-1" batch={batch([task(1, "SUCCEEDED", [
      artifact("asset-a", { name: "first.mp4", ordinal: 0 }),
      artifact("asset-b", { name: "second.mp4", ordinal: 1, availability: "missing" }),
    ])])} />);

    const first = screen.getByText("first.mp4").closest("article");
    const second = screen.getByText("second.mp4").closest("article");
    expect(first).toBeTruthy();
    expect(second).toBeTruthy();
    expect(within(first as HTMLElement).getByText("asset-a")).toBeTruthy();
    expect(within(first as HTMLElement).getByText("可用")).toBeTruthy();
    expect(within(second as HTMLElement).getByText("asset-b")).toBeTruthy();
    expect(within(second as HTMLElement).getByText("文件不存在")).toBeTruthy();
    await user.click(within(first as HTMLElement).getByRole("button", { name: "在文件夹中显示" }));
    await waitFor(() => expect(revealArtifact).toHaveBeenCalledWith("asset-a"));
    expect((within(second as HTMLElement).getByRole("button", { name: "打开" }) as HTMLButtonElement).disabled).toBe(true);
    expect(revealArtifact).not.toHaveBeenCalledWith("asset-b");
  });

  it("shows a typed open command failure to the user", async () => {
    const user = userEvent.setup();
    vi.mocked(openArtifact).mockRejectedValueOnce(new Error("ARTIFACT_OPEN_FAILED: opener failed"));
    render(<ProductionMonitor projectId="project-1" batch={batch([task(1, "SUCCEEDED", [artifact("asset-fail")])])} />);

    await user.click(screen.getByRole("button", { name: "打开" }));
    expect(await screen.findByRole("alert")).toBeTruthy();
  });

  it("does not treat task success as proof an artifact exists", () => {
    render(<ProductionMonitor projectId="project-1" batch={batch([task(1, "SUCCEEDED")])} />);
    expect(screen.getByText("任务已完成，但没有登记可用产物。")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "打开" })).toBeNull();
  });

  it("keeps production retry attached to the existing queue item", async () => {
    const user = userEvent.setup();
    const onRetry = vi.fn();
    render(<ProductionMonitor projectId="project-1" batch={batch([task(7, "FAILED", [], { errorCode: "TIMEOUT", errorMessage: "ComfyUI 超时" })], "FAILED")} onRetry={onRetry} />);

    await user.click(screen.getByRole("button", { name: "重试" }));
    expect(onRetry).toHaveBeenCalledWith("item-7");
    expect(screen.getByText("ComfyUI 超时")).toBeTruthy();
  });

  it("paginates task rows without collapsing their artifact collections", async () => {
    const user = userEvent.setup();
    const items = Array.from({ length: 51 }, (_, index) => task(index + 1, "SUCCEEDED", [artifact(`asset-${index + 1}`)]));
    render(<ProductionMonitor projectId="project-1" batch={batch(items)} />);
    expect(screen.getAllByRole("listitem")).toHaveLength(50);
    await user.click(screen.getByRole("button", { name: "下一页" }));
    expect(screen.getAllByRole("listitem")).toHaveLength(1);
    expect(screen.getByText("asset-51.mp4")).toBeTruthy();
  });
});
