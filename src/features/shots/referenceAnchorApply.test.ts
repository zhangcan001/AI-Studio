import { describe, expect, it } from "vitest";
import { ensurePrimaryReference, validateRef2vaReferences } from "./ShotWorkspace";
import {
  appendAnchorReferences,
  replaceWithAnchorReferences,
} from "./referenceAnchorApply";

describe("Shot reference anchor apply", () => {
  it("appends in current-then-anchor order and removes duplicates", () => {
    expect(appendAnchorReferences(["X", "B"], ["B", "A", "C"])).toEqual({
      ok: true,
      assetIds: ["X", "B", "A", "C"],
    });
  });

  it("replaces with the anchor order without sorting it", () => {
    expect(replaceWithAnchorReferences(["B", "A", "C"])).toEqual({
      ok: true,
      assetIds: ["B", "A", "C"],
    });
  });

  it("rejects a limit overflow instead of truncating", () => {
    const result = appendAnchorReferences(["A", "B"], ["C", "D"], 3);
    expect(result).toEqual({
      ok: false,
      assetIds: ["A", "B", "C", "D"],
      error: "参考图最多允许 3 张，未保存任何变更。",
    });
  });

  it("keeps the selected REF2VA key first after applying the ordered anchor", () => {
    const result = replaceWithAnchorReferences(["B", "KEY", "A"]);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(ensurePrimaryReference(result.assetIds, "KEY")).toEqual(["KEY", "B", "A"]);
  });

  it("validates the final REF2VA snapshot after applying an anchor", () => {
    const result = replaceWithAnchorReferences(["B", "A", "C"]);
    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(validateRef2vaReferences(
      { key: "reference_images", type: "images", label: "参考图", required: false, minItems: 2, maxItems: 3 },
      result.assetIds,
    )).toBeUndefined();
  });

  it("returns an independent snapshot rather than a live anchor reference", () => {
    const anchorAssetIds = ["A", "B", "C"];
    const result = replaceWithAnchorReferences(anchorAssetIds);
    anchorAssetIds[1] = "D";
    expect(result).toEqual({ ok: true, assetIds: ["A", "B", "C"] });
  });
});
