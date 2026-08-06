import { beforeEach, describe, expect, it } from "vitest";
import { useStudioStore } from "./studioStore";
import type { RecipeViewModel } from "../types/generation";

const mediaWorkflow: RecipeViewModel = {
  workflowId: "wfl_media",
  workflowVersionId: "wfv_media",
  recipeId: "rcp_media",
  name: "Media",
  category: "video",
  mode: "image_to_video",
  fields: [
    { key: "video", type: "video", label: "Video", required: true },
    { key: "audio", type: "audio", label: "Audio", required: false },
    { key: "videos", type: "videos", label: "Videos", required: false, minItems: 0, maxItems: 3 },
    { key: "audios", type: "audios", label: "Audios", required: false, minItems: 0, maxItems: 3 },
  ],
};

describe("studio media draft state", () => {
  beforeEach(() => {
    useStudioStore.getState().setSelectedWorkflow(undefined);
  });

  it("resets single and ordered media values with the selected workflow", () => {
    useStudioStore.getState().setSelectedWorkflow(mediaWorkflow);
    useStudioStore.getState().setValue("video", { type: "video_asset", assetId: "ast_video" });
    useStudioStore.getState().setValue("videos", { type: "video_assets", assetIds: ["ast_a", "ast_b"] });
    useStudioStore.getState().setSelectedWorkflow(mediaWorkflow);

    const values = useStudioStore.getState().values;
    expect(values.video).toBeUndefined();
    expect(values.audio).toBeUndefined();
    expect(values.videos).toEqual({ type: "video_assets", assetIds: [] });
    expect(values.audios).toEqual({ type: "audio_assets", assetIds: [] });
  });
});
