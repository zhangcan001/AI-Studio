// @vitest-environment jsdom

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
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

vi.mock("../../services/tauriClient", () => ({
  createProductionPackageBatches: vi.fn(),
  inspectProductionPackage: vi.fn(),
}));

const inspectMock = vi.mocked(inspectProductionPackage);
const createMock = vi.mocked(createProductionPackageBatches);

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
    expect(screen.getByText(/选择由外部智能体准备好的 Production Package 文件夹/)).toBeTruthy();
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
  });

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

  it("creates 150 selected items as two batches, never opens the queue automatically, and exposes a manual open callback", async () => {
    const user = userEvent.setup();
    const openQueue = vi.fn();
    const created = makeCreateResult(150);
    let resolveCreate: (result: ProductionPackageCreateBatchesResult) => void = () => undefined;
    inspectMock.mockResolvedValue(makeInspection(150, Array.from({ length: 150 }, () => "READY")));
    createMock.mockImplementation(() => new Promise((resolve) => { resolveCreate = resolve; }));
    render(<ProductionPackageWorkspace projectId="project-1" folderPath="C:/packages/ep01" onOpenQueue={openQueue} />);

    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(1));
    const createButton = screen.getByRole("button", { name: "创建生产批次（150 项）" }) as HTMLButtonElement;
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
    expect(openQueue).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "打开生产队列" }));
    await waitFor(() => expect(openQueue).toHaveBeenCalledTimes(1));
  });

  it("shows a media-change error and offers reinspection after create fails", async () => {
    const user = userEvent.setup();
    inspectMock
      .mockResolvedValueOnce(makeInspection(1, ["READY"]))
      .mockResolvedValueOnce(makeInspection(1, ["READY"]));
    createMock.mockRejectedValueOnce({ code: "PACKAGE_MEDIA_CHANGED", message: "changed" });
    render(<ProductionPackageWorkspace projectId="project-1" folderPath="C:/packages/ep01" />);

    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: /创建生产批次/ }));
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
    await user.click(screen.getByRole("button", { name: "创建生产批次（4 项）" }));
    await waitFor(() => expect(screen.getByRole("region", { name: "生产包创建结果" })).toBeTruthy());

    const createdRegion = screen.getByRole("region", { name: "生产包创建结果" });
    expect(screen.getByRole("region", { name: "Production Package 工作区" }).getAttribute("data-state")).toBe("PARTIAL");
    expect(within(createdRegion).getByText("已加入生产：2")).toBeTruthy();
    expect(within(createdRegion).getByText("尚未加入：2")).toBeTruthy();
    expect(within(createdRegion).getByText("状态：部分完成")).toBeTruthy();
    expect(within(createdRegion).getByText(/请求项目：4/)).toBeTruthy();
    expect(openQueue).not.toHaveBeenCalled();

    await user.click(within(createdRegion).getByRole("button", { name: "重新检查剩余项目" }));
    await waitFor(() => expect(inspectMock).toHaveBeenCalledTimes(2));
    expect((screen.getByRole("checkbox", { name: "选择项目 item-001" }) as HTMLInputElement).checked).toBe(false);
    expect((screen.getByRole("checkbox", { name: "选择项目 item-002" }) as HTMLInputElement).checked).toBe(false);
    expect((screen.getByRole("checkbox", { name: "选择项目 item-003" }) as HTMLInputElement).checked).toBe(true);
    expect((screen.getByRole("checkbox", { name: "选择项目 item-004" }) as HTMLInputElement).checked).toBe(false);
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
