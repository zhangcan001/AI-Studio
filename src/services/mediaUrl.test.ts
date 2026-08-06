import { describe, expect, it } from "vitest";
import { buildAssetMediaUrl } from "./mediaUrl";

describe("asset media URL", () => {
  it("contains only logical project and asset identifiers", () => {
    const url = buildAssetMediaUrl("project/one", "ast_video_1");
    expect(url).toBe("aistudio-media://localhost/video?projectId=project%2Fone&assetId=ast_video_1");
    expect(url).not.toContain("storage");
    expect(url).not.toContain("assets/");
  });
});
