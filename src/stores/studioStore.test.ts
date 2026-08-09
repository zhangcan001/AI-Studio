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

  it("loads an asset-free project template draft without inventing media references", () => {
    const workflow: RecipeViewModel = {
      ...mediaWorkflow,
      fields: [
        { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
        { key: "seed", type: "seed", label: "随机种子", defaultMode: "random" },
        { key: "video", type: "video", label: "参考视频", required: false },
      ],
    };
    useStudioStore.getState().loadDraft(workflow, {
      prompt: { type: "string", value: "人物海报" },
      seed: { type: "seed_fixed", value: "42" },
    });
    expect(useStudioStore.getState().values).toEqual({
      prompt: { type: "string", value: "人物海报" },
      seed: { type: "seed_fixed", value: "42" },
    });
    expect(useStudioStore.getState().values.video).toBeUndefined();
  });

  it("marks user edits so a preferred preset cannot overwrite an active draft", () => {
    useStudioStore.getState().loadDraft(mediaWorkflow, {});
    expect(useStudioStore.getState().draftDirty).toBe(false);
    useStudioStore.getState().setValue("note", { type: "string", value: "用户草稿" });
    expect(useStudioStore.getState().draftDirty).toBe(true);
    useStudioStore.getState().loadDraft(mediaWorkflow, {});
    expect(useStudioStore.getState().draftDirty).toBe(false);
  });
});
