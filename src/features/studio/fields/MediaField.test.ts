import { describe, expect, it } from "vitest";
import { isCompatibleAsset } from "./MediaField";
import { move } from "./MultiMediaField";
import type { AssetView } from "../../../types/asset";

function asset(assetType: string, category: string): AssetView {
  return {
    id: "ast_test",
    assetType,
    category,
    name: "test",
    originalName: "test",
    mimeType: `${assetType}/test`,
    fileSize: 1,
    createdAt: "2026-01-01T00:00:00Z",
  };
}

describe("media field compatibility", () => {
  it("accepts source and generated video but only source audio", () => {
    expect(isCompatibleAsset(asset("video", "source_video"), "video")).toBe(true);
    expect(isCompatibleAsset(asset("video", "generated_video"), "video")).toBe(true);
    expect(isCompatibleAsset(asset("audio", "source_audio"), "audio")).toBe(true);
    expect(isCompatibleAsset(asset("image", "source_image"), "audio")).toBe(false);
  });

  it("keeps ordered multi-media operations deterministic", () => {
    expect(move(["ast_a", "ast_b", "ast_c"], 2, 0)).toEqual(["ast_c", "ast_a", "ast_b"]);
  });
});
