// @vitest-environment jsdom

import { cleanup, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ToolCapabilityView,
  ToolInstanceView,
  ToolVersionView,
  ToolView,
} from "../../types/tool";
import { LocalToolHub } from "./LocalToolHub";

const mocks = vi.hoisted(() => ({
  listTools: vi.fn(),
  listToolInstances: vi.fn(),
  listToolVersions: vi.fn(),
  listToolCapabilities: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, ...mocks };
});

const tool: ToolView = {
  id: "tool_comfy",
  name: "ComfyUI",
  type: "image",
  description: "本地图像与视频工作流运行时",
  metadata: { managed: false },
  createdAt: "2026-09-01T00:00:00Z",
};

const instance: ToolInstanceView = {
  id: "tins_comfy",
  toolId: tool.id,
  path: "C:/ComfyUI",
  endpoint: "http://127.0.0.1:8188",
  status: "AVAILABLE",
  lastChecked: "2026-09-02T00:00:00Z",
};

const version: ToolVersionView = {
  id: "tver_comfy",
  toolId: tool.id,
  version: "0.3.0",
  observedAt: "2026-09-02T00:00:00Z",
  metadata: { source: "manual" },
};

const capability: ToolCapabilityView = {
  toolId: tool.id,
  capabilityName: "image_generation",
  metadata: { available: true },
};

describe("LocalToolHub", () => {
  beforeEach(() => {
    mocks.listTools.mockResolvedValue([tool]);
    mocks.listToolInstances.mockResolvedValue([instance]);
    mocks.listToolVersions.mockResolvedValue([version]);
    mocks.listToolCapabilities.mockResolvedValue([capability]);
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("renders the typed tool list with status, version, and location", async () => {
    render(<LocalToolHub />);

    expect(await screen.findByRole("button", { name: "ComfyUI" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "名称" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "类型" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "状态" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "版本" })).toBeTruthy();
    expect(screen.getByRole("columnheader", { name: "位置" })).toBeTruthy();
    expect(screen.getAllByText("可用").length).toBeGreaterThan(0);
    expect(screen.getAllByText("v0.3.0").length).toBeGreaterThan(0);
    expect(screen.getAllByText("C:/ComfyUI · http://127.0.0.1:8188").length).toBeGreaterThan(0);
    expect(mocks.listTools).toHaveBeenCalledTimes(1);
    expect(mocks.listToolInstances).toHaveBeenCalledWith(tool.id);
    expect(mocks.listToolVersions).toHaveBeenCalledWith(tool.id);
    expect(mocks.listToolCapabilities).toHaveBeenCalledWith(tool.id);
  });

  it("shows the selected detail, capabilities, observed versions, and health", async () => {
    render(<LocalToolHub />);

    const detail = await screen.findByRole("region", { name: "工具详情" });
    expect(within(detail).getByText("本地图像与视频工作流运行时")).toBeTruthy();
    expect(within(detail).getByText("image_generation")).toBeTruthy();
    expect(within(detail).getByText("观察版本历史")).toBeTruthy();
    expect(within(detail).getByText("健康状态")).toBeTruthy();
    expect(within(detail).getByText(/最近记录：/)).toBeTruthy();
    expect(within(detail).getByText("状态来自最近一次显式记录，不执行端点探测、进程启动或停止。")).toBeTruthy();
  });

  it("surfaces a missing-tool state without attempting to run anything", async () => {
    const missingInstance: ToolInstanceView = {
      ...instance,
      path: null,
      endpoint: null,
      status: "MISSING",
      lastChecked: null,
    };
    mocks.listToolInstances.mockResolvedValue([missingInstance]);
    render(<LocalToolHub />);

    expect(await screen.findByText("工具位置缺失或最近一次记录不可用；本页面不会自动启动或安装工具。")).toBeTruthy();
    expect(screen.getAllByText("缺失").length).toBeGreaterThan(0);
    expect(screen.getAllByText("未登记位置").length).toBeGreaterThan(0);
  });

  it("exposes loading, empty, and list error states", async () => {
    mocks.listTools.mockImplementationOnce(() => new Promise(() => undefined));
    render(<LocalToolHub />);
    expect(screen.getByText("正在加载本地工具…")).toBeTruthy();

    cleanup();
    mocks.listTools.mockRejectedValueOnce(new Error("tool registry unavailable"));
    render(<LocalToolHub />);
    expect(await screen.findByText("本地工具列表加载失败：操作失败，请查看技术详情。")).toBeTruthy();

    cleanup();
    mocks.listTools.mockResolvedValueOnce([]);
    render(<LocalToolHub />);
    expect(await screen.findByText("暂无本地工具登记。")).toBeTruthy();
  });

  it("does not collapse a detail request failure into unknown health", async () => {
    mocks.listToolInstances.mockRejectedValueOnce(new Error("instance detail unavailable"));
    render(<LocalToolHub />);

    expect((await screen.findAllByText("加载失败")).length).toBeGreaterThan(0);
    expect(screen.getByText("工具详情加载失败：操作失败，请查看技术详情。")).toBeTruthy();
    expect(screen.queryByText("未知")).toBeNull();
  });
});
