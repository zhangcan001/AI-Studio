import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import { MINIMAX_H3_FL2VA_WORKFLOW_ID } from "../runtime/productRuntimeScope";
import { AssetVideoBatchWorkspace } from "./AssetVideoBatchWorkspace";

const fl2vaRecipe: RecipeViewModel = {
  workflowId: MINIMAX_H3_FL2VA_WORKFLOW_ID,
  workflowVersionId: "wfv_fl2va",
  recipeId: "rcp_fl2va",
  name: "MiniMax H3 FL2VA",
  category: "video",
  mode: "fl2va",
  fields: [
    { key: "duration_seconds", type: "integer", label: "时长", required: true, default: 5, min: 1, max: 15, step: 1 },
    { key: "width", type: "integer", label: "宽度", required: true, default: 1344, min: 32, max: 2048, step: 32 },
    { key: "height", type: "integer", label: "高度", required: true, default: 768, min: 32, max: 2048, step: 32 },
    { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
    { key: "first_frame", type: "image", label: "首帧", required: false },
    { key: "last_frame", type: "image", label: "末帧", required: false },
    { key: "seed", type: "seed", label: "种子", defaultMode: "random" },
  ],
  outputTypes: ["video"],
};

describe("H3 批量视频工作区渲染安全", () => {
  it("renders direct navigation with no assets and defaults to text-to-video", () => {
    const html = renderToStaticMarkup(
      <AssetVideoBatchWorkspace
        projectId="project-1"
        catalog={[fl2vaRecipe]}
        initialAssets={[]}
        comfyConnected
        taskEventsReady
        productionAdmission={{ busy: false }}
        onAdmissionChanged={vi.fn().mockResolvedValue(undefined)}
        onProductionBatchFocused={vi.fn()}
        onOpenTask={vi.fn()}
        onBackToAssets={vi.fn()}
      />,
    );

    expect(html).toContain("批量视频");
    expect(html).toContain("文生视频");
    expect(html).toContain("视频 Prompt");
    expect(html).toMatch(/<button[^>]*disabled[^>]*>创建视频批次（0）<\/button>/);
    expect(html).not.toContain("REF2VA_IMAGE");
  });
});
