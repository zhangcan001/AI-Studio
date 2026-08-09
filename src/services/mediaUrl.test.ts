import { describe, expect, it } from "vitest";
import { buildAssetMediaUrl } from "./mediaUrl";

describe("asset media URL", () => {
  it("contains only logical project and asset identifiers", () => {
    const url = buildAssetMediaUrl("project/one", "ast_video_1");
    expect(url).toBe("aistudio-media://localhost/video?projectId=project%2Fone&assetId=ast_video_1");
    expect(url).not.toContain("storage");
    expect(url).not.toContain("assets/");
  });

  it("uses a separate audio protocol route", () => {
    expect(buildAssetMediaUrl("prj_default", "ast_audio", "audio"))
      .toBe("aistudio-media://localhost/audio?projectId=prj_default&assetId=ast_audio");
  });

  it("uses Wry's HTTP custom-protocol mapping on Windows", () => {
    expect(buildAssetMediaUrl("prj_default", "ast_video", "video", "Windows NT 10.0"))
      .toBe("http://aistudio-media.localhost/video?projectId=prj_default&assetId=ast_video");
  });
});
