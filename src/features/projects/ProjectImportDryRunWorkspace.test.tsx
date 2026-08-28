// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  commitShotBulkImport,
  previewShotBulkImport,
} from "../../services/tauriClient";
import { ProjectImportDryRunWorkspace } from "./ProjectImportDryRunWorkspace";

vi.mock("../../services/tauriClient", () => ({
  commitShotBulkImport: vi.fn(),
  previewShotBulkImport: vi.fn(),
}));

const preview = (overrides: Record<string, unknown> = {}) => ({
  total: 2,
  valid: 2,
  invalid: 0,
  warnings: 0,
  rows: [
    { rowNumber: 1, name: "镜头 01", description: "入口", imagePrompt: "暗色", videoPrompt: "推进", errors: [], warnings: [] },
    { rowNumber: 2, name: "镜头 02", description: "出口", imagePrompt: "亮色", videoPrompt: "拉远", errors: [], warnings: [] },
  ],
  ...overrides,
});

function uploadFile(name: string, value: string, type = "application/json") {
  return new File([value], name, { type });
}

describe("ProjectImportDryRunWorkspace", () => {
  const user = () => userEvent.setup();

  beforeEach(() => {
    vi.mocked(previewShotBulkImport).mockReset();
    vi.mocked(commitShotBulkImport).mockReset();
    vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("starts empty and exposes the project-level dry-run workspace", () => {
    render(<ProjectImportDryRunWorkspace projectId="project-1" onClose={vi.fn()} />);
    expect(screen.getByRole("region", { name: "批量导入预检工作区" })).toBeTruthy();
    expect(screen.getByRole("heading", { name: "批量导入 / 导入预检" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "运行预检" })).toHaveProperty("disabled", true);
    expect(screen.queryByText("导入文件摘要")).toBeNull();
  });

  it("reads a JSON file, previews it, and executes once only after confirmation", async () => {
    vi.mocked(previewShotBulkImport).mockResolvedValue(preview());
    vi.mocked(commitShotBulkImport).mockResolvedValue({
      projectId: "project-1",
      created: [
        { shotId: "shot-1", ordinal: 0, name: "镜头 01" },
        { shotId: "shot-2", ordinal: 1, name: "镜头 02" },
      ],
    });
    const onImported = vi.fn();
    const actor = user();
    render(<ProjectImportDryRunWorkspace projectId="project-1" onClose={vi.fn()} onImported={onImported} />);

    await actor.upload(screen.getByLabelText("选择 JSON / TSV 文件"), uploadFile("shots.json", "{\"shots\":[]}"));
    await actor.click(screen.getByRole("button", { name: "运行预检" }));

    await waitFor(() => expect(previewShotBulkImport).toHaveBeenCalledWith({
      projectId: "project-1",
      format: "json",
      content: "{\"shots\":[]}",
    }));
    expect(screen.getAllByText("预检通过")).toHaveLength(2);
    expect(screen.getByText("预计新增 2 个镜头。现有导入接口为 CREATE ONLY，不会更新或删除已有数据。")).toBeTruthy();
    expect(screen.getByRole("button", { name: "确认导入" })).toHaveProperty("disabled", false);

    await actor.click(screen.getByRole("button", { name: "确认导入" }));
    await waitFor(() => expect(commitShotBulkImport).toHaveBeenCalledTimes(1));
    expect(commitShotBulkImport).toHaveBeenCalledWith({
      projectId: "project-1",
      format: "json",
      content: "{\"shots\":[]}",
    });
    expect(onImported).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("status", { name: "导入完成" })).toBeTruthy();
    expect(screen.getByText("已写入 2 个镜头；本次没有自动生成任务。")).toBeTruthy();
  });

  it("keeps execution disabled when the preview has a blocking row error", async () => {
    vi.mocked(previewShotBulkImport).mockResolvedValue(preview({
      total: 2,
      valid: 1,
      invalid: 1,
      rows: [
        { rowNumber: 1, name: "镜头 01", description: "入口", imagePrompt: "暗色", videoPrompt: "推进", errors: [], warnings: [] },
        { rowNumber: 2, name: "重复镜头", description: "出口", imagePrompt: "亮色", videoPrompt: "拉远", errors: [{ code: "DUPLICATE_NAME", message: "名称已存在", rowNumber: 2, shotId: "shot-2" }], warnings: [] },
      ],
    }));
    const actor = user();
    render(<ProjectImportDryRunWorkspace projectId="project-1" onClose={vi.fn()} />);

    await actor.upload(screen.getByLabelText("选择 JSON / TSV 文件"), uploadFile("shots.tsv", "镜头 01\t入口", "text/tab-separated-values"));
    await actor.click(screen.getByRole("button", { name: "运行预检" }));

    await waitFor(() => expect(screen.getAllByText("预检阻塞")).toHaveLength(2));
    expect(screen.getByText("DUPLICATE_NAME")).toBeTruthy();
    expect(screen.getByText("第 2 行")).toBeTruthy();
    expect(screen.getByRole("button", { name: "确认导入" })).toHaveProperty("disabled", true);
    expect(commitShotBulkImport).not.toHaveBeenCalled();
  });

  it("shows malformed backend errors and reset removes stale results", async () => {
    vi.mocked(previewShotBulkImport).mockRejectedValue({ code: "BULK_IMPORT_INVALID_JSON", message: "JSON 结构无效" });
    const actor = user();
    render(<ProjectImportDryRunWorkspace projectId="project-1" onClose={vi.fn()} />);

    await actor.upload(screen.getByLabelText("选择 JSON / TSV 文件"), uploadFile("broken.json", "not-json"));
    await actor.click(screen.getByRole("button", { name: "运行预检" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("JSON 结构无效"));
    expect(screen.getAllByText("BULK_IMPORT_INVALID_JSON")).toHaveLength(2);

    await actor.click(screen.getByRole("button", { name: "清除并重新选择" }));
    expect(screen.queryByText("BULK_IMPORT_INVALID_JSON")).toBeNull();
    expect(screen.queryByText("导入文件摘要")).toBeNull();
    expect(screen.getByRole("button", { name: "运行预检" })).toHaveProperty("disabled", true);
  });

  it("shows a failed execution without rendering a false success", async () => {
    vi.mocked(previewShotBulkImport).mockResolvedValue(preview({ total: 1, valid: 1, rows: [preview().rows[0]] }));
    vi.mocked(commitShotBulkImport).mockRejectedValue({ code: "BULK_IMPORT_COMMIT_FAILED", message: "写入失败" });
    const actor = user();
    render(<ProjectImportDryRunWorkspace projectId="project-1" onClose={vi.fn()} />);

    await actor.upload(screen.getByLabelText("选择 JSON / TSV 文件"), uploadFile("shots.json", "{\"shots\":[]}"));
    await actor.click(screen.getByRole("button", { name: "运行预检" }));
    await actor.click(await screen.findByRole("button", { name: "确认导入" }));

    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("写入失败"));
    expect(screen.queryByRole("status", { name: "导入完成" })).toBeNull();
    expect(commitShotBulkImport).toHaveBeenCalledTimes(1);
  });

  it("rejects unsupported and empty files before calling the backend", async () => {
    const actor = userEvent.setup({ applyAccept: false });
    render(<ProjectImportDryRunWorkspace projectId="project-1" onClose={vi.fn()} />);

    await actor.upload(screen.getByLabelText("选择 JSON / TSV 文件"), uploadFile("shots.csv", "a,b", "text/csv"));
    expect(screen.getByRole("alert").textContent).toContain("当前只支持 JSON 或 TSV/TXT");
    expect(previewShotBulkImport).not.toHaveBeenCalled();

    await actor.upload(screen.getByLabelText("选择 JSON / TSV 文件"), uploadFile("empty.json", ""));
    expect(screen.getByRole("alert").textContent).toContain("导入文件为空");
    expect(previewShotBulkImport).not.toHaveBeenCalled();
  });
});
