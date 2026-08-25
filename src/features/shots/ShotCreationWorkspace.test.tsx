import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { AssetView } from "../../types/asset";
import type { ShotView } from "../../types/shot";
import { ShotCreationWorkspace, canConfirmShotCandidate, resolveShotPreviewAsset, type ShotWorkspaceCandidate } from "./ShotCreationWorkspace";

const asset = (id: string, type: "image" | "video"): AssetView => ({
  id,
  assetType: type,
  category: type === "image" ? "generated_image" : "generated_video",
  name: `${type}-${id}`,
  originalName: `${id}.media`,
  mimeType: type === "image" ? "image/png" : "video/mp4",
  width: 1280,
  height: 720,
  durationMs: type === "video" ? 8000 : null,
  fileSize: 1024,
  createdAt: "2026-08-25T00:00:00Z",
  thumbnailAvailable: true,
  isFavorite: false,
  tags: [],
});

const shot: ShotView = {
  id: "shot-1",
  projectId: "project-1",
  ordinal: 3,
  name: "业火焚烧",
  promptText: "slow cinematic fire and smoke",
  createdAt: "2026-08-25T00:00:00Z",
  updatedAt: "2026-08-25T00:00:00Z",
  status: "DRAFT",
  imageStatus: "IMAGE_REVIEW",
  videoStatus: "VIDEO_REVIEW",
  stageConfigs: [],
  referenceAssets: [],
  generationLinks: [],
};

const candidates: ShotWorkspaceCandidate[] = [
  { asset: asset("video-1", "video"), status: "ready", taskId: "task-1" },
  { asset: asset("video-2", "video"), status: "failed", error: "渲染超时" },
];

describe("ShotCreationWorkspace", () => {
  it("renders the shot header, stage switch, workspace tabs, preview rail, prompt preview, and inspector contract", () => {
    const html = renderToStaticMarkup(
      <ShotCreationWorkspace
        projectId="project-1"
        shot={shot}
        stage="video"
        onStageChange={vi.fn()}
        candidates={candidates}
        selectedAssetId="video-1"
        onCandidateSelect={vi.fn()}
        onCandidateConfirm={vi.fn()}
        onGenerate={vi.fn()}
        promptText={shot.promptText}
        onPromptChange={vi.fn()}
        currentRecipe={{ workflowId: "h3", workflowVersionId: "h3-v1", recipeId: "h3-r1", name: "H3 Quality", category: "video", mode: "reference_to_video", fields: [{ key: "steps", type: "integer", label: "Steps", required: true, default: 20 }] }}
      />,
    );

    expect(html).toContain("业火焚烧");
    expect(html).toContain("图片");
    expect(html).toContain("视频");
    expect(html).toContain("生成");
    expect(html).toContain("参考");
    expect(html).toContain("历史");
    expect(html).toContain("设置");
    expect(html).toContain("候选");
    expect(html).toContain("提示词预览");
    expect(html).toContain("参数");
    expect(html).toContain("preload=\"metadata\"");
    expect(html).toContain("controls=\"\"");
    expect(html).not.toContain("zoomable-image-toolbar");
    expect((html.match(/shot-candidate-confirm/g) ?? []).length).toBe(1);
    expect(html).not.toContain(">确认<");
    expect((html.match(/shot-inspector-generate/g) ?? []).length).toBe(1);
  });

  it("uses the selected asset for preview without mutating selection, then falls back to the latest candidate", () => {
    const selected = resolveShotPreviewAsset(candidates, "video-2");
    expect(selected?.id).toBe("video-2");
    expect(resolveShotPreviewAsset(candidates, "missing")?.id).toBe("video-1");
    expect(resolveShotPreviewAsset(candidates, "missing", asset("manual-1", "image"))?.id).toBe("manual-1");
  });

  it("keeps thumbnail preview selection separate from explicit confirmation", () => {
    expect(canConfirmShotCandidate(candidates[0], "video-2")).toBe(true);
    expect(canConfirmShotCandidate(candidates[0], "video-1")).toBe(false);
    expect(canConfirmShotCandidate({ ...candidates[0], status: "reviewed" }, "other")).toBe(false);
  });

  it("keeps a real empty state when there is no selected shot", () => {
    const html = renderToStaticMarkup(<ShotCreationWorkspace projectId="project-1" stage="image" onStageChange={vi.fn()} onGenerate={vi.fn()} onCreateShot={vi.fn()} />);
    expect(html).toContain("选择一个镜头开始制作");
    expect(html).toContain("新建镜头");
  });
});
