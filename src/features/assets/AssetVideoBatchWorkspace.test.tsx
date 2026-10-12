import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import { MINIMAX_H3_FL2VA_WORKFLOW_ID } from "../runtime/productRuntimeScope";
import { AssetVideoBatchWorkspace, ProjectFolderImportControls, h3InitialGenerationMode, h3PickerAssets, projectParameterSourceLabel } from "./AssetVideoBatchWorkspace";
import type { AssetView } from "../../types/asset";
function asset(id: string, assetType: "image" | "video" | "audio", category: AssetView["category"]): AssetView {
  return { id, assetType, category, name: id, originalName: `${id}.bin`, mimeType: `${assetType}/test`, width: assetType === "image" ? 1344 : undefined, height: assetType === "image" ? 768 : undefined, durationMs: assetType === "image" ? undefined : 1000, fileSize: 100, createdAt: "2026-08-12T00:00:00Z", thumbnailAvailable: assetType !== "audio", isFavorite: false, tags: [] };
}
const imageA = asset("image-a", "image", "source_image");
const imageB = asset("image-b", "image", "generated_image");
const videoA = asset("video-a", "video", "source_video");
const audioA = asset("audio-a", "audio", "source_audio");
const fl2vaRecipe: RecipeViewModel = { workflowId: MINIMAX_H3_FL2VA_WORKFLOW_ID, workflowVersionId: "wfv_fl2va", recipeId: "rcp_fl2va", name: "MiniMax H3 FL2VA", category: "video", mode: "fl2va", fields: [{ key: "duration_seconds", type: "integer", label: "时长", required: true, default: 5, min: 1, max: 15, step: 1 }, { key: "width", type: "integer", label: "宽度", required: true, default: 1344, min: 32, max: 2048, step: 32 }, { key: "height", type: "integer", label: "高度", required: true, default: 768, min: 32, max: 2048, step: 32 }, { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" }, { key: "first_frame", type: "image", label: "首帧", required: false }, { key: "last_frame", type: "image", label: "末帧", required: false }, { key: "seed", type: "seed", label: "种子", defaultMode: "random" }], outputTypes: ["video"] };
describe("H3 批量视频工作区已停用本地权威", () => {
  it("exposes only the project-folder local import entry helpers", () => {
    const html = renderToStaticMarkup(<ProjectFolderImportControls busy={false} hasInspection onRescan={vi.fn()} />);
    expect(html).toContain("项目文件夹 · 分段自动识别");
    expect(html).toContain("重新扫描");
  });
  it("labels per-segment parameter sources for confirmation", () => {
    expect(projectParameterSourceLabel("PROMPT_SPEC")).toBe("提示词");
  });
  it("renders deprecation shell toward ShotBatchAssetBindPanel without create/auto-start", () => {
    const html = renderToStaticMarkup(<AssetVideoBatchWorkspace projectId="project-1" catalog={[fl2vaRecipe]} initialAssets={[]} comfyConnected taskEventsReady productionAdmission={{ busy: false }} onAdmissionChanged={vi.fn().mockResolvedValue(undefined)} onProductionBatchFocused={vi.fn()} onOpenTask={vi.fn()} onBackToAssets={vi.fn()} />);
    expect(html).toContain("请改用镜头正式输入槽");
    expect(html).toContain("ShotBatchAssetBindPanel");
    expect(html).toContain("已停用：本地输入权威 / 自动开跑");
    expect(html).not.toContain("创建视频批次");
    expect(html).not.toContain("加入队列并开始");
    expect(html).not.toContain("导入后立即开始生成");
  });
  it("keeps helper defaults for single preselected image mode", () => {
    expect(h3InitialGenerationMode([imageA])).toBe("FL2VA_IMAGE_TO_VIDEO");
  });
  it("filters source and generated media by assetType for every H3 picker mode", () => {
    const assets = [imageA, imageB, videoA, audioA];
    expect(h3PickerAssets(assets, "FL2VA_IMAGE_TO_VIDEO").map((i) => i.id)).toEqual(["image-a", "image-b"]);
    expect(h3PickerAssets(assets, "REF2VA_AUDIO").map((i) => i.id)).toEqual(["audio-a"]);
  });
});
