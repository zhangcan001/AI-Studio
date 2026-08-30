// @vitest-environment jsdom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  createProductionPackageBatches,
  inspectProductionPackage,
} from "../../services/tauriClient";
import type {
  ProductionPackageCreateBatchesResult,
  ProductionPackageInspectionResult,
} from "../../services/tauriClient";
import type { ProductionPackageInspectionItem, ProductionPackageItemStatus } from "../../types/productionPackage";
import { ProductionPackageWorkspace } from "./ProductionPackageWorkspace";

const dropState = vi.hoisted(() => ({
  handler: undefined as ((event: { payload: DragDropEvent }) => void) | undefined,
  subscribe: vi.fn(),
  unlisten: vi.fn(),
}));

vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: async (handler: (event: { payload: DragDropEvent }) => void) => {
      dropState.handler = handler;
      dropState.subscribe();
      return dropState.unlisten;
    },
  }),
}));

vi.mock("../../services/tauriClient", () => ({
  createProductionPackageBatches: vi.fn(),
  inspectProductionPackage: vi.fn(),
}));

const inspectMock = vi.mocked(inspectProductionPackage);
const createMock = vi.mocked(createProductionPackageBatches);

function emitDrop(payload: DragDropEvent) {
  dropState.handler?.({ payload });
}

function emitDropPaths(paths: string[]) {
  emitDrop({ type: "drop", paths, position: {} as DragDropEvent extends { position: infer Position } ? Position : never });
}

beforeEach(() => {
  dropState.handler = undefined;
  dropState.subscribe.mockClear();
  dropState.unlisten.mockClear();
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("ProductionPackageWorkspace", () => {
  it("starts EMPTY and automatically inspects a parent-provided folder", async () => {
    inspectMock.mockResolvedValue(makeInspection(1, ["READY"]));
    const { rerender } = render(<ProductionPackageWorkspace projectId="project-1" />);

    expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-state")).toBe("EMPTY");
    expect(screen.getByRole("heading", { name: "批量视频生产" })).toBeTruthy();
    expect(screen.getByText(/选择或拖入外部智能体准备好的 Production Package 文件夹/)).toBeTruthy();
    expect(screen.getByText("Production Package V1 规范 / 生产包格式说明")).toBeTruthy();
    rerender(<ProductionPackageWorkspace projectId="project-1" folderPath="C:/packages/ep01" />);

    await waitFor(() => expect(inspectMock).toHaveBeenCalledWith("project-1", "C:/packages/ep01"));
    expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-state")).toBe("READY");
  });

  it("keeps 500 items paged at 50, defaults READY selection, allows WARNING, and disables BLOCKED", async () => {
    const user = userEvent.setup();
    inspectMock.mockResolvedValue(makeInspection(500));
    render(<ProductionPackageWorkspace projectId="project-1" folderPath="C:/packages/large" />);

    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(1));
    await user.click(screen.getByText("查看 500 个镜头明细"));
    const table = screen.getByRole("table", { name: "生产包项目列表" });
    expect(within(table).getAllByRole("row")).toHaveLength(51);
    expect(screen.getByText(/每页 50/)).toBeTruthy();

    const ready = within(table).getByRole("checkbox", { name: "选择项目 item-001" }) as HTMLInputElement;
    const warning = within(table).getByRole("checkbox", { name: "选择项目 item-002" }) as HTMLInputElement;
    const blocked = within(table).getByRole("checkbox", { name: "选择项目 item-003" }) as HTMLInputElement;
    expect(ready.checked).toBe(true);
    expect(warning.checked).toBe(false);
    expect(blocked.disabled).toBe(true);

    await user.click(warning);
    expect(warning.checked).toBe(true);
    expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-selected-count")).toBe("401");

    await user.click(screen.getByRole("button", { name: "下一页" }));
    expect(screen.getByLabelText("第 2 / 10 页")).toBeTruthy();
    const secondPageWarning = screen.getByRole("checkbox", { name: "选择项目 item-052" }) as HTMLInputElement;
    expect(secondPageWarning.checked).toBe(false);
    await user.click(secondPageWarning);
    expect(secondPageWarning.checked).toBe(true);

    await user.click(screen.getByRole("button", { name: "上一页" }));
    await user.click(screen.getByRole("button", { name: "下一页" }));
    expect((screen.getByRole("checkbox", { name: "选择项目 item-052" }) as HTMLInputElement).checked).toBe(true);

    await user.click(screen.getByRole("button", { name: /WARNING 50/ }));
    expect((screen.getByRole("checkbox", { name: "选择项目 item-052" }) as HTMLInputElement).checked).toBe(true);
    expect(screen.getAllByText(/已选择 402 项（/).length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "清空选择" }));
    expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-selected-count")).toBe("0");
    await user.click(screen.getByRole("button", { name: "全选 READY" }));
    expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-selected-count")).toBe("400");
  }, 10_000);

  it("reinspects the current folder and resets the selection to the new READY set", async () => {
    const user = userEvent.setup();
    inspectMock
      .mockResolvedValueOnce(makeInspection(3, ["READY", "WARNING", "BLOCKED"]))
      .mockResolvedValueOnce(makeInspection(3, ["READY", "READY", "WARNING"]));
    render(<ProductionPackageWorkspace projectId="project-1" folderPath="C:/packages/ep01" />);

    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("checkbox", { name: "选择项目 item-002" }));
    expect(screen.getAllByText(/已选择 2 项（/).length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "重新检查" }));
    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(2));
    expect((screen.getByRole("checkbox", { name: "选择项目 item-001" }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("checkbox", { name: "选择项目 item-002" }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("checkbox", { name: "选择项目 item-003" }) as HTMLInputElement).checked).toBe(false);
  });

  it("automatically inspects a path returned by the folder picker", async () => {
    const user = userEvent.setup();
    const picker = vi.fn().mockResolvedValue("C:/packages/picked");
    inspectMock.mockResolvedValue(makeInspection(1, ["READY"]));
    render(<ProductionPackageWorkspace projectId="project-1" onChooseFolder={picker} />);

    await user.click(screen.getByRole("button", { name: "选择生产包文件夹" }));
    await waitFor(() => expect(inspectMock).toHaveBeenCalledWith("project-1", "C:/packages/picked"));
    expect(picker).toHaveBeenCalledTimes(1);
    const folderPathInput = screen.getByLabelText("Production Package 文件夹路径") as HTMLInputElement;
    expect(folderPathInput.readOnly).toBe(true);
    expect(folderPathInput.getAttribute("aria-readonly")).toBe("true");
  });

  it("shows the complete selected path and a truthful inspection status", async () => {
    inspectMock.mockResolvedValue(makeInspection(1, ["READY"]));
    const fullPath = "D:\\AI漫剧\\第一集\\生产包";
    render(<ProductionPackageWorkspace projectId="project-1" folderPath={fullPath} />);

    await waitFor(() => expect(inspectMock).toHaveBeenCalledWith("project-1", fullPath));
    const folderPathInput = screen.getByLabelText("Production Package 文件夹路径") as HTMLInputElement;
    expect(folderPathInput.value).toBe(fullPath);
    expect(folderPathInput.title).toBe(fullPath);
    expect(screen.getByText("已选择 · 检查完成")).toBeTruthy();
  });

  it("creates 150 selected items as two batches and automatically opens the queue without starting it", async () => {
    const user = userEvent.setup();
    const openQueue = vi.fn();
    const created = makeCreateResult(150);
    let resolveCreate: (result: ProductionPackageCreateBatchesResult) => void = () => undefined;
    inspectMock.mockResolvedValue(makeInspection(150, Array.from({ length: 150 }, () => "READY")));
    createMock.mockImplementation(() => new Promise((resolve) => { resolveCreate = resolve; }));
    render(<ProductionPackageWorkspace projectId="project-1" folderPath="C:/packages/ep01" onOpenProductionQueue={openQueue} />);

    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(1));
    const createButton = screen.getByRole("button", { name: "创建并打开生产队列（150 项）" }) as HTMLButtonElement;
    await user.click(createButton);
    await waitFor(() => expect(createMock).toHaveBeenCalledWith(
      "inspection-150",
      Array.from({ length: 150 }, (_, index) => `item-${String(index + 1).padStart(3, "0")}`),
    ));
    expect(createButton.disabled).toBe(true);
    expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-state")).toBe("CREATING_BATCHES");

    resolveCreate(created);
    await waitFor(() => expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-state")).toBe("CREATED"));
    expect(screen.getByText("已创建 2 个生产批次")).toBeTruthy();
    const createdRegion = screen.getByRole("region", { name: "生产包创建结果" });
    expect(within(createdRegion).getByText("150 个项目")).toBeTruthy();
    expect(within(createdRegion).getByText(/自动启动：否/)).toBeTruthy();
    await waitFor(() => expect(openQueue).toHaveBeenCalledWith(created));
    expect(openQueue).toHaveBeenCalledTimes(1);
    expect(screen.getByText("生产批次已创建并已打开生产队列；不会自动开始生成。")).toBeTruthy();
    expect(screen.queryByText("批次已创建；不会自动打开或启动生产队列。")).toBeNull();
    expect((screen.getByRole("button", { name: "批次已创建" }) as HTMLButtonElement).disabled).toBe(true);
  });

  it("shows a media-change error and offers reinspection after create fails", async () => {
    const user = userEvent.setup();
    inspectMock
      .mockResolvedValueOnce(makeInspection(1, ["READY"]))
      .mockResolvedValueOnce(makeInspection(1, ["READY"]));
    createMock.mockRejectedValueOnce({ code: "PACKAGE_MEDIA_CHANGED", message: "changed" });
    render(<ProductionPackageWorkspace projectId="project-1" folderPath="C:/packages/ep01" />);

    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: /创建并打开生产队列/ }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("媒体文件已变化");
    expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-state")).toBe("ERROR");

    await user.click(screen.getByRole("button", { name: "重新检查" }));
    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(2));
  });

  it("shows partial truth and reinspects only the remaining external IDs", async () => {
    const user = userEvent.setup();
    const openQueue = vi.fn();
    inspectMock
      .mockResolvedValueOnce(makeInspection(4, ["READY", "READY", "READY", "READY"]))
      .mockResolvedValueOnce(makeInspection(4, ["READY", "READY", "READY", "WARNING"]));
    createMock.mockResolvedValueOnce(makePartialCreateResult());
    render(<ProductionPackageWorkspace projectId="project-1" folderPath="C:/packages/ep01" onOpenQueue={openQueue} />);

    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: "创建并打开生产队列（4 项）" }));
    await waitFor(() => expect(screen.getByRole("region", { name: "生产包创建结果" })).toBeTruthy());

    const createdRegion = screen.getByRole("region", { name: "生产包创建结果" });
    expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-state")).toBe("PARTIAL");
    expect(within(createdRegion).getByText("已加入生产：2")).toBeTruthy();
    expect(within(createdRegion).getByText("尚未加入：2")).toBeTruthy();
    expect(within(createdRegion).getByText("状态：部分完成")).toBeTruthy();
    expect(within(createdRegion).getByText(/请求项目：4/)).toBeTruthy();
    await waitFor(() => expect(openQueue).toHaveBeenCalledTimes(1));
    expect(within(createdRegion).getByRole("button", { name: "打开已创建队列" })).toBeTruthy();

    await user.click(within(createdRegion).getByRole("button", { name: "重新检查剩余项目" }));
    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(2));
    expect((screen.getByRole("checkbox", { name: "选择项目 item-001" }) as HTMLInputElement).checked).toBe(false);
    expect((screen.getByRole("checkbox", { name: "选择项目 item-002" }) as HTMLInputElement).checked).toBe(false);
    expect((screen.getByRole("checkbox", { name: "选择项目 item-003" }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("checkbox", { name: "选择项目 item-004" }) as HTMLInputElement).checked).toBe(false);
  });

  it("accepts one dropped folder, auto-inspects it, and rejects files or multiple paths", async () => {
    inspectMock.mockResolvedValue(makeInspection(1, ["READY"]));
    render(<ProductionPackageWorkspace projectId="project-1" />);

    await waitFor(() => expect(dropState.subscribe).toHaveBeenCalledTimes(1));
    emitDropPaths(["D:\\AI漫剧\\EP01\\生产包"]);
    await waitFor(() => expect(inspectMock).toHaveBeenCalledWith("project-1", "D:\\AI漫剧\\EP01\\生产包"));
    expect(inspectMock).toHaveBeenCalledTimes(1);

    cleanup();
    dropState.handler = undefined;
    inspectMock.mockClear();
    render(<ProductionPackageWorkspace projectId="project-1" />);
    await waitFor(() => expect(dropState.subscribe).toHaveBeenCalledTimes(2));
    emitDropPaths(["D:\\AI漫剧\\EP01\\生产包", "D:\\AI漫剧\\EP02\\生产包"]);
    emitDropPaths(["D:\\AI漫剧\\EP01\\生产包\\production-package.json"]);
    expect(inspectMock).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("请拖入包含 production-package.json 的整个 Production Package 文件夹"));
  });

  it("keeps a successful create when opening the queue fails and reopens without creating again", async () => {
    const user = userEvent.setup();
    const openQueue = vi.fn().mockRejectedValueOnce({ code: "QUEUE_UNAVAILABLE", message: "offline" });
    createMock.mockResolvedValueOnce(makeCreateResult(1));
    inspectMock.mockResolvedValue(makeInspection(1, ["READY"]));
    render(<ProductionPackageWorkspace projectId="project-1" folderPath="C:/packages/ep01" onOpenQueue={openQueue} />);

    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: "创建并打开生产队列（1 项）" }));
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("生产批次已创建，但生产队列暂时无法打开");
    expect(screen.getByText("批次已经创建，可重新打开生产队列；不会重复创建批次。")).toBeTruthy();
    expect(screen.queryByText(/再次创建批次|重新创建批次/)).toBeNull();
    expect(screen.getByRole("button", { name: "重新打开生产队列" })).toBeTruthy();
    expect((screen.getByRole("button", { name: "批次已创建" }) as HTMLButtonElement).disabled).toBe(true);
    expect(createMock).toHaveBeenCalledTimes(1);

    openQueue.mockResolvedValueOnce(undefined);
    await user.click(screen.getByRole("button", { name: "重新打开生产队列" }));
    await waitFor(() => expect(openQueue).toHaveBeenCalledTimes(2));
    expect(createMock).toHaveBeenCalledTimes(1);
    expect(screen.getByText("生产批次已创建并已打开生产队列；不会自动开始生成。")).toBeTruthy();
  });

  it("clears only the package workspace for the next package", async () => {
    const user = userEvent.setup();
    createMock.mockResolvedValueOnce(makeCreateResult(1));
    inspectMock.mockResolvedValue(makeInspection(1, ["READY"]));
    render(<ProductionPackageWorkspace projectId="project-1" defaultFolderPath="C:/packages/ep01" />);

    await user.click(screen.getByRole("button", { name: "检查文件夹" }));
    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: "创建并打开生产队列（1 项）" }));
    await screen.findByRole("region", { name: "生产包创建结果" });
    expect(screen.getByText("生产批次已创建；不会自动开始生成。")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "选择下一个生产包" }));

    expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-state")).toBe("EMPTY");
    expect((screen.getByLabelText("Production Package 文件夹路径") as HTMLInputElement).value).toBe("");
    expect(createMock).toHaveBeenCalledTimes(1);
  });
});

function makeInspection(count: number, statuses?: ProductionPackageItemStatus[]): ProductionPackageInspectionResult {
  const resolvedStatuses = statuses ?? Array.from({ length: count }, (_, index) => {
    if (index % 10 === 1) return "WARNING";
    if (index % 10 === 2) return "BLOCKED";
    return "READY";
  });
  const items = resolvedStatuses.map((status, index): ProductionPackageInspectionItem => ({
    id: `item-${String(index + 1).padStart(3, "0")}`,
    name: `镜头 ${index + 1}`,
    mode: "FL2VA_IMAGE_TO_VIDEO",
    videoPromptPreview: "镜头缓慢推进。",
    duration: 5,
    resolution: { width: 864, height: 480 },
    status,
    warnings: status === "WARNING" ? [{ code: "PACKAGE_MODE_ALIAS", message: "使用兼容模式别名" }] : [],
    errors: status === "BLOCKED" ? [{ code: "PACKAGE_MEDIA_MISSING", message: "媒体文件缺失" }] : [],
  }));
  return {
    inspectionId: `inspection-${count}`,
    packageName: "EP01 · 雨夜",
    packageType: "AI_STUDIO_VIDEO_PRODUCTION",
    itemCount: items.length,
    readyCount: items.filter((item) => item.status === "READY").length,
    warningCount: items.filter((item) => item.status === "WARNING").length,
    blockedCount: items.filter((item) => item.status === "BLOCKED").length,
    items,
  };
}

function makeCreateResult(itemCount: number): ProductionPackageCreateBatchesResult {
  return {
    packageName: "EP01 · 雨夜",
    status: "COMPLETE",
    requestedCount: itemCount,
    createdCount: itemCount,
    remainingCount: 0,
    remainingItemIds: [],
    batchCount: 2,
    itemCount,
    autoStarted: false,
    batches: [
      { batchId: "batch-1", batchName: "EP01 · 雨夜 · 1/2", itemCount: 100, itemMappings: [] },
      { batchId: "batch-2", batchName: "EP01 · 雨夜 · 2/2", itemCount: 50, itemMappings: [] },
    ],
    itemMappings: [],
    warnings: [],
  };
}

function makePartialCreateResult(): ProductionPackageCreateBatchesResult {
  return {
    packageName: "EP01 · 雨夜",
    status: "PARTIAL",
    requestedCount: 4,
    createdCount: 2,
    remainingCount: 2,
    remainingItemIds: ["item-003", "item-004"],
    batchCount: 1,
    itemCount: 2,
    autoStarted: false,
    batches: [
      { batchId: "batch-1", batchName: "EP01 · 雨夜 · 1/1", itemCount: 2, itemMappings: [] },
    ],
    itemMappings: [],
    warnings: [],
  };
}
