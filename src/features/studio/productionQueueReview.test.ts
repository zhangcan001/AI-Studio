import { describe, expect, it } from "vitest";
import { hasReviewableVideoOutput, isReviewableVideoAsset } from "./productionQueueReview";

describe("production queue review eligibility", () => {
  it("recognizes generated video output even when runtime ids are opaque", () => {
    expect(isReviewableVideoAsset({ assetType: "video", category: "generated_image", mimeType: "application/octet-stream" })).toBe(true);
    expect(isReviewableVideoAsset({ assetType: "asset", category: "generated_video", mimeType: "application/octet-stream" })).toBe(true);
    expect(isReviewableVideoAsset({ assetType: "asset", category: "generated_asset", mimeType: "video/mp4" })).toBe(true);
  });

  it("does not expose review for image-only output", () => {
    expect(isReviewableVideoAsset({ assetType: "image", category: "generated_image", mimeType: "image/png" })).toBe(false);
    expect(hasReviewableVideoOutput({ item1: [] })).toBe(false);
  });
});
