import { describe, expect, it } from "vitest";
import type { ShotView } from "../../types/shot";
import { shotImagesReady } from "./ShotProgressDashboard";

const baseShot = (overrides: Partial<ShotView> = {}): ShotView => ({
  id: "sht-1",
  projectId: "prj-1",
  ordinal: 0,
  name: "镜头",
  promptText: "画面",
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
  status: "DRAFT",
  imageStatus: "DRAFT",
  videoStatus: "DRAFT",
  stageConfigs: [],
  referenceAssets: [],
  generationLinks: [],
  ...overrides,
});

describe("Shot progress image readiness", () => {
  it("counts generated candidates and selected keyframes as image-ready", () => {
    expect(shotImagesReady([
      baseShot({ selectedImageAssetId: "ast-selected" }),
      baseShot({
        id: "sht-2",
        generationLinks: [{
          id: "lnk-2",
          stage: "image",
          createdAt: "2026-01-01T00:00:00Z",
          task: { id: "tsk-2", status: "SUCCEEDED", outputAssetIds: ["ast-candidate"] } as ShotView["generationLinks"][number]["task"],
        }],
      }),
    ])).toBe(2);
  });
});
