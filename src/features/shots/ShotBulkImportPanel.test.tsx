import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  commitShotBulkImport,
  previewShotBulkImport,
  ShotBulkImportPanel,
  shotBulkImportRowClassName,
  type ShotBulkImportRowPreview,
} from "./ShotBulkImportPanel";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const request = { projectId: "project-1", format: "tsv" as const, content: "镜头 01\t地狱入口" };

const row = (overrides: Partial<ShotBulkImportRowPreview> = {}): ShotBulkImportRowPreview => ({
  rowNumber: 1,
  name: "镜头 01",
  description: "地狱入口",
  imagePrompt: "暗色电影光",
  videoPrompt: "镜头缓慢推进",
  errors: [],
  warnings: [],
  ...overrides,
});

describe("ShotBulkImportPanel", () => {
  it("exposes TSV and JSON tabs, preview counts, and a guarded confirm action", () => {
    const html = renderToStaticMarkup(<ShotBulkImportPanel projectId="project-1" />);

    expect(html).toContain("批量导入镜头");
    expect(html).toContain('role="tablist"');
    expect(html).toContain(">TSV<");
    expect(html).toContain(">JSON<");
    expect(html).toContain("总行数：");
    expect(html).toContain("可导入：");
    expect(html).toContain("错误：");
    expect(html).toContain("警告：");
    expect(html).toContain("检查");
    expect(html).toContain("确认导入");
    expect(html).toContain("取消");
    expect(html).toMatch(/确认导入[\s\S]*disabled/);
  });

  it("highlights rows with validation errors while leaving warnings non-blocking", () => {
    expect(shotBulkImportRowClassName(row({ errors: ["名称重复"] }))).toBe("shot-batch-row-blocked");
    expect(shotBulkImportRowClassName(row({ warnings: ["提示词较短"] }))).toBe("");
  });

  it("uses the shared preview and commit command envelope without reparsing the import contract", async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ total: 1, valid: 1, invalid: 0, warnings: 0, rows: [row()] });
    await previewShotBulkImport(request);
    expect(invoke).toHaveBeenLastCalledWith("preview_shot_bulk_import", { request });

    vi.mocked(invoke).mockResolvedValueOnce(undefined);
    await commitShotBulkImport(request);
    expect(invoke).toHaveBeenLastCalledWith("commit_shot_bulk_import", { request });
  });
});
