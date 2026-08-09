import { describe, expect, it } from "vitest";
import type { AssetView } from "../../types/asset";
import { applyAssetPickerAction, buildAssetPickerQuery, filterPickerAssets, toggleAssetSelection } from "./assetPicker";

const assets = [
  { id: "img-source", assetType: "image", category: "source_image", name: "source", fileSize: 1 },
  { id: "img-generated", assetType: "image", category: "generated_image", name: "generated", fileSize: 1 },
  { id: "video-source", assetType: "video", category: "source_video", name: "video", fileSize: 1 },
] as AssetView[];

describe("素材选择器", () => {
  it("只按当前项目返回兼容类型并支持源素材/生成素材筛选", () => {
    expect(filterPickerAssets(assets, "image", "all").map((asset) => asset.id)).toEqual([
      "img-source",
      "img-generated",
    ]);
    expect(filterPickerAssets(assets, "image", "source").map((asset) => asset.id)).toEqual(["img-source"]);
    expect(filterPickerAssets(assets, "video", "generated")).toEqual([]);
  });

  it("单选替换，多选保留顺序并受上限约束", () => {
    expect(toggleAssetSelection(["a"], "b", false, 1)).toEqual(["b"]);
    expect(toggleAssetSelection(["a"], "b", true, 2)).toEqual(["a", "b"]);
    expect(toggleAssetSelection(["a", "b"], "c", true, 2)).toEqual(["a", "b"]);
    expect(toggleAssetSelection(["a", "b"], "a", true, 2)).toEqual(["b"]);
  });

  it("使用后端查询限制媒体类型、来源和关键词，并支持游标分页", () => {
    const cursor = { createdAt: "2026-01-01T00:00:00Z", id: "asset-1" };
    expect(buildAssetPickerQuery("project-1", "image", "source", " 人物 ", true, "tag-person", cursor)).toEqual({
      projectId: "project-1",
      category: "ALL",
      keyword: "人物",
      mediaType: "IMAGE",
      sourceKind: "SOURCE",
      favoriteOnly: true,
      tagId: "tag-person",
      createdOrder: "NEWEST",
      cursor,
      limit: 30,
    });
  });

  it("取消不改变已提交选择，确定才提交选择器草稿", () => {
    const committed = ["existing"];
    const draft = ["existing", "new"];

    expect(applyAssetPickerAction(committed, draft, "cancel")).toEqual(["existing"]);
    expect(applyAssetPickerAction(committed, draft, "confirm")).toEqual(draft);
  });
});
