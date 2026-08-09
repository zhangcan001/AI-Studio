import { describe, expect, it } from "vitest";
import type { AssetView } from "../../types/asset";
import { filterPickerAssets, toggleAssetSelection } from "./assetPicker";

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
});
