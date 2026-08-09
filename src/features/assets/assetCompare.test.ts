import { describe, expect, it } from "vitest";
import type { AssetView } from "../../types/asset";
import { toggleCompareSelection } from "./assetCompare";

const asset = (id: string, assetType: "image" | "video" | "audio"): AssetView => ({
  id,
  assetType,
  category: `${assetType === "audio" ? "source" : "generated"}_${assetType}`,
  name: id,
  originalName: `${id}.file`,
  mimeType: `${assetType}/test`,
  fileSize: 1,
  createdAt: "2026-01-01T00:00:00Z",
  isFavorite: false,
  tags: [],
});

describe("资产对比选择", () => {
  it("允许2到4个同类型图片或视频并保留顺序", () => {
    const first = asset("image-1", "image");
    const second = asset("image-2", "image");
    const result = toggleCompareSelection([first], second);
    expect(result.assets.map((item) => item.id)).toEqual(["image-1", "image-2"]);
    expect(result.notice).toBeUndefined();
  });

  it("阻止音频、混合类型和第五个素材", () => {
    const image = asset("image", "image");
    expect(toggleCompareSelection([], asset("audio", "audio")).notice).toContain("音频");
    expect(toggleCompareSelection([image], asset("video", "video")).notice).toContain("相同类型");
    const four = [1, 2, 3, 4].map((index) => asset(`image-${index}`, "image"));
    expect(toggleCompareSelection(four, asset("image-5", "image")).notice).toContain("4");
  });

  it("再次选择已选素材会移除它", () => {
    const image = asset("image", "image");
    expect(toggleCompareSelection([image], image).assets).toEqual([]);
  });
});
