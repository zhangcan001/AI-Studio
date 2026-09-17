// @vitest-environment jsdom

import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getArtifactReviewQueue, submitArtifactReview } from "../../services/tauriClient";
import type { ArtifactDto, ArtifactReviewDecision, ArtifactReviewQueueItemDto, ArtifactReviewQueuePageDto } from "../../types/artifact";
import { ProductionReviewInbox } from "./ProductionReviewInbox";

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return {
    ...actual,
    getArtifactReviewQueue: vi.fn(),
    submitArtifactReview: vi.fn(),
    openArtifact: vi.fn(),
    revealArtifact: vi.fn(),
    getAssetMediaUrl: vi.fn((_projectId: string, assetId: string) => `asset://${assetId}`),
  };
});

afterEach(() => { cleanup(); vi.clearAllMocks(); });

function item(
  id: string,
  availability: ArtifactDto["availability"] = "available",
  decision: ArtifactReviewDecision = "PENDING",
  comment = "",
  revision = decision === "PENDING" ? 0 : 1,
): ArtifactReviewQueueItemDto {
  return {
    artifact: {
      id,
      taskId: `task-${id}`,
      outputId: "output-video",
      ordinal: 0,
      mediaType: "video",
      name: `${id}.mp4`,
      mimeType: "video/mp4",
      width: 1280,
      height: 720,
      durationMs: 4000,
      sizeBytes: 3000,
      version: 1,
      createdAt: "2026-09-17T00:00:00Z",
      thumbnailAvailable: false,
      availability,
      reviewStatus: decision,
    },
    task: {
      id: `task-${id}`,
      status: "SUCCEEDED",
      workflowVersionId: "wfv-1",
      recipeId: "rcp-1",
      createdAt: "2026-09-17T00:00:00Z",
      finishedAt: "2026-09-17T00:01:00Z",
    },
    review: {
      id: `review-${id}`,
      artifactId: id,
      decision,
      comment,
      revision,
      createdAt: "2026-09-17T00:00:00Z",
      updatedAt: "2026-09-17T00:00:00Z",
    },
  };
}

function page(items: ArtifactReviewQueueItemDto[], total = items.length): ArtifactReviewQueuePageDto {
  return { items, total, limit: 50, offset: 0 };
}

function setQueuePages(
  pending: () => ArtifactReviewQueuePageDto | Promise<ArtifactReviewQueuePageDto>,
  completed: () => ArtifactReviewQueuePageDto | Promise<ArtifactReviewQueuePageDto> = () => page([]),
) {
  vi.mocked(getArtifactReviewQueue).mockImplementation(async (_projectId, _limit, _offset, filter) =>
    filter === "completed" ? completed() : pending());
}

beforeEach(() => {
  setQueuePages(() => page([item("asset-a")]));
  vi.mocked(submitArtifactReview).mockImplementation(async (request) => ({
    id: `review-${request.artifactId}`,
    artifactId: request.artifactId,
    decision: request.decision,
    comment: request.comment,
    revision: request.expectedRevision + 1,
    createdAt: "2026-09-17T00:00:00Z",
    updatedAt: "2026-09-17T00:02:00Z",
  }));
});

describe("ProductionReviewInbox", () => {
  it("loads a typed pending Artifact queue with media preview and task context", async () => {
    render(<ProductionReviewInbox projectId="project-1" mode="workspace" />);

    expect(await screen.findByRole("heading", { name: "产物审核队列" })).toBeTruthy();
    expect(screen.getAllByText("asset-a.mp4")).toHaveLength(2);
    expect(screen.getByLabelText("asset-a.mp4")).toBeTruthy();
    expect(screen.getByText("wfv-1")).toBeTruthy();
    expect(screen.getAllByText("待审核")).toHaveLength(2);
    expect(getArtifactReviewQueue).toHaveBeenCalledWith("project-1", 50, 0);
    expect(getArtifactReviewQueue).toHaveBeenCalledWith("project-1", 50, 0, "completed");
  });

  it("approves the selected artifact and automatically selects the next pending artifact", async () => {
    const user = userEvent.setup();
    let pendingItems = [item("asset-a"), item("asset-b")];
    let completedItems: ArtifactReviewQueueItemDto[] = [];
    setQueuePages(
      () => page(pendingItems),
      () => page(completedItems),
    );
    vi.mocked(submitArtifactReview).mockImplementationOnce(async (request) => {
      pendingItems = [item("asset-b")];
      completedItems = [item("asset-a", "available", "APPROVED", "", 1)];
      return {
        id: `review-${request.artifactId}`,
        artifactId: request.artifactId,
        decision: request.decision,
        comment: request.comment,
        revision: request.expectedRevision + 1,
        createdAt: "2026-09-17T00:00:00Z",
        updatedAt: "2026-09-17T00:02:00Z",
      };
    });
    render(<ProductionReviewInbox projectId="project-1" mode="workspace" />);

    await screen.findByRole("heading", { name: "asset-a.mp4" });
    const approve = screen.getByRole("button", { name: "通过" });
    await user.click(approve);
    await waitFor(() => expect(submitArtifactReview).toHaveBeenCalledWith({
      projectId: "project-1",
      artifactId: "asset-a",
      decision: "APPROVED",
      comment: "",
      expectedRevision: 0,
    }));
    expect(screen.getByRole("heading", { name: "asset-b.mp4" })).toBeTruthy();
    expect(within(screen.getByLabelText("待审核产物列表")).queryByText("asset-a.mp4")).toBeNull();
    const historyItem = await screen.findByTestId("artifact-review-history-asset-a");
    expect(within(historyItem).getByText("asset-a.mp4")).toBeTruthy();
    expect(within(historyItem).getByText("已通过")).toBeTruthy();
    expect(within(historyItem).getByText("无备注")).toBeTruthy();
    expect(within(historyItem).getByText("asset-a")).toBeTruthy();
    expect(within(historyItem).queryByRole("button", { name: "通过" })).toBeNull();
    expect(within(historyItem).queryByRole("button", { name: "驳回" })).toBeNull();
  });

  it("requires a rejection comment and submits the exact artifact and revision", async () => {
    const user = userEvent.setup();
    let pendingItems = [item("asset-reject")];
    setQueuePages(() => page(pendingItems));
    vi.mocked(submitArtifactReview).mockImplementationOnce(async (request) => {
      pendingItems = [];
      return {
        id: `review-${request.artifactId}`,
        artifactId: request.artifactId,
        decision: request.decision,
        comment: request.comment,
        revision: request.expectedRevision + 1,
        createdAt: "2026-09-17T00:00:00Z",
        updatedAt: "2026-09-17T00:02:00Z",
      };
    });
    render(<ProductionReviewInbox projectId="project-1" mode="workspace" />);

    await screen.findByRole("heading", { name: "asset-reject.mp4" });
    const reject = await screen.findByRole("button", { name: "驳回" });
    expect((reject as HTMLButtonElement).disabled).toBe(true);
    await user.type(screen.getByLabelText("审核备注（驳回时必填）"), "参考图主体未保留");
    expect((reject as HTMLButtonElement).disabled).toBe(false);
    await user.click(reject);
    await waitFor(() => expect(submitArtifactReview).toHaveBeenCalledWith({
      projectId: "project-1",
      artifactId: "asset-reject",
      decision: "REJECTED",
      comment: "参考图主体未保留",
      expectedRevision: 0,
    }));
    expect(await screen.findByText(/没有待审核产物/)).toBeTruthy();
  });

  it("shows persisted approved and rejected review history with isolated comments and artifact IDs", async () => {
    const rejectionComment = "测试驳回：画面不符合预期。";
    setQueuePages(
      () => page([item("asset-pending")]),
      () => page([
        item("artifact-approved", "available", "APPROVED", "", 1),
        item("artifact-rejected", "missing", "REJECTED", rejectionComment, 1),
      ]),
    );
    render(<ProductionReviewInbox projectId="project-1" mode="workspace" />);

    const approved = await screen.findByTestId("artifact-review-history-artifact-approved");
    const rejected = await screen.findByTestId("artifact-review-history-artifact-rejected");
    expect(within(approved).getByText("已通过")).toBeTruthy();
    expect(within(approved).getByText("artifact-approved")).toBeTruthy();
    expect(within(approved).getByText("无备注")).toBeTruthy();
    expect(within(approved).getByText("Revision").parentElement?.textContent).toContain("1");
    expect(within(approved).getByText("可用")).toBeTruthy();
    expect(within(rejected).getByText("已驳回")).toBeTruthy();
    expect(within(rejected).getByText("artifact-rejected")).toBeTruthy();
    expect(within(rejected).getByText(rejectionComment)).toBeTruthy();
    expect(within(rejected).getByText("文件不存在")).toBeTruthy();
    expect(within(rejected).queryByRole("button", { name: "通过" })).toBeNull();
    expect(within(rejected).queryByRole("button", { name: "驳回" })).toBeNull();
    expect(getArtifactReviewQueue).toHaveBeenCalledWith("project-1", 50, 0, "completed");
  });

  it("shows unavailable files and prevents review actions", async () => {
    setQueuePages(() => page([item("asset-missing", "missing")]));
    render(<ProductionReviewInbox projectId="project-1" mode="workspace" />);

    await screen.findByRole("heading", { name: "asset-missing.mp4" });
    expect(screen.getByText("文件不存在；文件可用前不能审核。")).toBeTruthy();
    expect((screen.getByRole("button", { name: "通过" }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole("button", { name: "驳回" }) as HTMLButtonElement).disabled).toBe(true);
    expect(submitArtifactReview).not.toHaveBeenCalled();
  });

  it("displays empty queue and load errors rather than synthesizing review state", async () => {
    let failNextPending = false;
    setQueuePages(() => {
      if (failNextPending) {
        failNextPending = false;
        return Promise.reject(new Error("offline"));
      }
      return page([]);
    });
    const view = render(<ProductionReviewInbox projectId="project-1" mode="workspace" />);
    expect(await screen.findByText(/没有待审核产物/)).toBeTruthy();

    failNextPending = true;
    await userEvent.setup().click(screen.getByRole("button", { name: "刷新队列" }));
    expect(await screen.findByRole("alert")).toBeTruthy();
    view.unmount();
  });

  it("navigates from the compact summary to the project review queue", async () => {
    const user = userEvent.setup();
    vi.mocked(getArtifactReviewQueue).mockResolvedValue(page([item("asset-a")], 27));
    const onNavigate = vi.fn();
    render(<ProductionReviewInbox projectId="project-a" onNavigate={onNavigate} />);

    await user.click(await screen.findByRole("button", { name: "查看审核队列" }));
    expect(onNavigate).toHaveBeenCalledWith({
      destination: "shots",
      projectId: "project-a",
      section: "review",
      collectionFilter: { kind: "review", state: "PENDING" },
    });
  });

  it("renders a loading state before the review queue response resolves", async () => {
    let resolveQueue: ((value: ArtifactReviewQueuePageDto) => void) | undefined;
    setQueuePages(() => new Promise((resolve) => { resolveQueue = resolve; }));
    render(<ProductionReviewInbox projectId="project-1" mode="workspace" />);
    expect(screen.getByText("正在加载审核队列…")).toBeTruthy();
    await act(async () => { resolveQueue?.(page([])); });
    expect(await screen.findByText(/没有待审核产物/)).toBeTruthy();
  });
});
