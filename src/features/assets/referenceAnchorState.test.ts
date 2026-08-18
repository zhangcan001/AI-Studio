import { describe, expect, it } from "vitest";
import type { AssetView } from "../../types/asset";
import type { ReferenceAnchorAssetView, ReferenceAnchorView } from "../../types/referenceAnchor";
import { appendUniqueReferenceAssets, filterReferenceAnchors, orderedReferenceAnchorAssetIds } from "./referenceAnchorState";

const asset = (id: string): AssetView => ({
  id, assetType: "image", category: "source_image", name: id, originalName: `${id}.png`, mimeType: "image/png",
  fileSize: 1, createdAt: "2026-08-18T00:00:00Z", isFavorite: false, tags: [],
});

const anchor = (id: string, name: string, kind: ReferenceAnchorView["kind"]): ReferenceAnchorView => ({
  id, projectId: "project-1", kind, name, description: `${name} description`, assets: [], usable: false,
  createdAt: "2026-08-18T00:00:00Z", updatedAt: "2026-08-18T00:00:00Z",
});

describe("参考锚点前端状态", () => {
  it("keeps anchor assets in ordinal order and filters by kind/name", () => {
    const items: ReferenceAnchorAssetView[] = [
      { assetId: "B", ordinal: 1 },
      { assetId: "A", ordinal: 0 },
    ];
    expect(orderedReferenceAnchorAssetIds(items)).toEqual(["A", "B"]);
    expect(filterReferenceAnchors([anchor("1", "地藏菩萨", "CHARACTER"), anchor("2", "天宫", "SCENE")], "CHARACTER", "地藏")).toHaveLength(1);
  });

  it("deduplicates selected images and caps the ordered list at 20", () => {
    const selected = Array.from({ length: 21 }, (_, index) => asset(`asset-${index}`));
    const result = appendUniqueReferenceAssets([{ assetId: "asset-0", ordinal: 0 }], selected);
    expect(result).toHaveLength(20);
    expect(result.map((item) => item.assetId).filter((id) => id === "asset-0")).toHaveLength(1);
    expect(result.map((item) => item.ordinal)).toEqual(Array.from({ length: 20 }, (_, index) => index));
  });
});
