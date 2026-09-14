// @vitest-environment jsdom

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AssetRelationView, AssetVersionView, AssetView } from "../../types/asset";
import { AssetPreview } from "./AssetPreview";

const mocks = vi.hoisted(() => ({
  getAssetUsage: vi.fn(),
  getAssetVideoPrompt: vi.fn(),
  listAssetRelations: vi.fn(),
  listAssetVersions: vi.fn(),
  listGenerationAssetVersionLinks: vi.fn(),
  listGenerationToolUsages: vi.fn(),
  readAssetImage: vi.fn(),
}));

vi.mock("../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient");
  return { ...actual, ...mocks };
});

const asset: AssetView = {
  id: "asset-1",
  assetType: "image",
  category: "generated_image",
  name: "生成图片 1",
  originalName: "generated.png",
  mimeType: "image/png",
  fileSize: 1024,
  createdAt: "2026-09-01T00:00:00Z",
  updatedAt: "2026-09-02T00:00:00Z",
  sourceTaskId: "task-1",
  isFavorite: false,
  tags: [{ id: "tag-character", name: "人物" }],
};

const versions: AssetVersionView[] = [
  { id: "av-1", projectId: "project-1", assetId: "asset-1", versionNumber: 1, createdAt: "2026-09-01T00:00:00Z" },
  { id: "av-2", projectId: "project-1", assetId: "asset-1", versionNumber: 2, createdAt: "2026-09-02T00:00:00Z" },
];

const relations: AssetRelationView[] = [{
  id: "rel-1",
  projectId: "project-1",
  sourceAssetId: "asset-1",
  targetAssetId: "asset-2",
  sourceAssetName: "生成图片 1",
  targetAssetName: "变体图片",
  relationType: "VARIANT_OF",
  createdAt: "2026-09-02T00:00:00Z",
}];

describe("AssetPreview detail MVP", () => {
  beforeEach(() => {
    mocks.getAssetUsage.mockResolvedValue({
      assetId: "asset-1",
      total: 0,
      blockingCount: 0,
      referenceSets: [],
      profiles: [],
      shots: [],
      legacyReferences: [],
      productionHistory: [],
      items: [],
    });
    mocks.getAssetVideoPrompt.mockResolvedValue(null);
    mocks.listAssetRelations.mockResolvedValue(relations);
    mocks.listAssetVersions.mockResolvedValue(versions);
    mocks.listGenerationAssetVersionLinks.mockResolvedValue([]);
    mocks.listGenerationToolUsages.mockResolvedValue([]);
    mocks.readAssetImage.mockRejectedValue(new Error("preview unavailable"));
  });

  afterEach(() => {
    cleanup();
    vi.clearAllMocks();
  });

  it("shows metadata, provenance, relations, and read-only version history", async () => {
    render(<AssetPreview projectId="project-1" asset={asset} onClose={vi.fn()} />);

    expect(screen.getByLabelText("素材元数据")).toBeTruthy();
    expect(screen.getByLabelText("素材来源与溯源")).toBeTruthy();
    await waitFor(() => expect(screen.getAllByText("v2").length).toBeGreaterThan(0));
    expect(screen.getByText("变体图片")).toBeTruthy();
    expect(screen.getByText("人物")).toBeTruthy();
    expect(screen.getByText("任务 task-1")).toBeTruthy();
    expect(screen.getByText("版本历史")).toBeTruthy();
    expect(screen.getByText("资产关系")).toBeTruthy();
    expect(screen.getByText("Prompt")).toBeTruthy();
    expect(screen.getByText("Model")).toBeTruthy();
    expect(screen.getByText("Generation")).toBeTruthy();
    expect(screen.getByText("Date")).toBeTruthy();
  });

  it("shows explicit tool and asset-version provenance without inferring missing links", async () => {
    mocks.listGenerationToolUsages.mockResolvedValue([{
      id: "gtu-1",
      generationId: "task-1",
      toolInstanceId: "tins-comfy",
      toolVersionId: "tver-comfy-1",
      metadata: { source: "explicit" },
      createdAt: "2026-09-02T01:00:00Z",
    }]);
    mocks.listGenerationAssetVersionLinks.mockResolvedValue([{
      id: "gav-1",
      generationId: "task-1",
      outputId: "output-0",
      ordinal: 0,
      assetVersionId: "av-2",
      relationType: "OUTPUT",
      createdAt: "2026-09-02T01:01:00Z",
    }]);

    render(<AssetPreview projectId="project-1" asset={asset} onClose={vi.fn()} />);

    expect((await screen.findAllByText("tver-comfy-1")).length).toBeGreaterThan(0);
    expect(screen.getByText("av-2")).toBeTruthy();
    expect(screen.getByText("未记录（历史数据未提供显式提示词版本关联）")).toBeTruthy();
    expect(screen.getByText("未记录（历史数据未提供显式模型版本关联）")).toBeTruthy();
    expect(mocks.listGenerationToolUsages).toHaveBeenCalledWith("project-1", "task-1");
    expect(mocks.listGenerationAssetVersionLinks).toHaveBeenCalledWith("project-1", "task-1");
  });
});
