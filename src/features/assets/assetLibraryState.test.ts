import { describe, expect, it } from "vitest";
import { mergeAssetPage } from "./assetLibraryState";

const asset = (id: string) => ({
  id,
  category: "generated_image",
  name: id,
  originalName: `${id}.png`,
  mimeType: "image/png",
  width: 1,
  height: 1,
  fileSize: 1,
  createdAt: "2026-01-01T00:00:00Z",
});

describe("asset library pagination state", () => {
  it("keeps project page results unique", () => {
    expect(mergeAssetPage([asset("ast_1")], [asset("ast_1"), asset("ast_2")], false).map((item) => item.id)).toEqual([
      "ast_1",
      "ast_2",
    ]);
  });
});
