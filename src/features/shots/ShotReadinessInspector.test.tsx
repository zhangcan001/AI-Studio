import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ShotProductionPlanDetail } from "../../types/productionPreparation";
import { readinessCheckMessages, readinessGateEntries } from "./ShotReadinessInspector";
import { ShotReadinessInspector } from "./ShotReadinessInspector";

const detail: ShotProductionPlanDetail = {
  projectId: "project-1",
  shotId: "shot-1",
  ordinal: 4,
  name: "雨夜入口",
  sceneId: "scene-1",
  stage: "video",
  contextHash: "0123456789abcdef0123456789abcdef",
  resolvedContext: {
    stage: "video",
    profiles: {
      characters: [{ profileId: "character-1", profileType: "Character", source: { scope: "SCENE", scopeId: "scene-1" } }],
      scene: { profileId: "scene-profile-1", profileType: "Scene", source: { scope: "PROJECT", scopeId: "project-1" } },
      props: [],
      style: null,
    },
    referencePack: {
      referenceSets: [{ referenceSetId: "reference-set-1", role: "CHARACTER", ordinal: 0, source: { scope: "SHOT", scopeId: "shot-1" } }],
    },
    referenceAssets: [{ assetId: "asset-1", sha256: "sha-1", sourceReferenceSetId: "reference-set-1", role: "CHARACTER", ordinal: 0 }],
    workflow: { workflowVersionId: "workflow-1", recipeId: "recipe-1" },
    stageInput: { selectedImageAssetId: "keyframe-1", selectedImageSha256: "keyframe-sha" },
    legacy: { hasReferencePack: false, usesLegacyShotReferences: false },
  },
  readiness: {
    projectId: "project-1",
    shotId: "shot-1",
    stage: "video",
    status: "BLOCKED",
    score: 65,
    contextHash: "0123456789abcdef0123456789abcdef",
    gates: [
      { key: "CHARACTER", state: "PASS", checks: [] },
      { key: "SCENE", state: "PASS", checks: [] },
      { key: "REFERENCE", state: "PASS", checks: [] },
      { key: "PROMPT", state: "PASS", checks: [] },
      { key: "WORKFLOW", state: "PASS", checks: [] },
      { key: "OUTPUT", state: "PASS", checks: [] },
      { key: "COMFY_CAPABILITY", state: "BLOCKER", checks: [{ state: "BLOCKER", code: "COMFY_OFFLINE", message: "ComfyUI 离线" }] },
    ],
  },
  currentStageStatus: "未开始",
  existingBatchIds: [],
  matchingPreparedBatchIds: [],
  stalePreparedBatchIds: ["batch-old"],
  alreadyPrepared: false,
  blockers: ["ComfyUI 离线"],
  warnings: [],
};

describe("ShotReadinessInspector", () => {
  it("shows all seven gates, context hash, profile sources, reference summary, and offline blocker", () => {
    const html = renderToStaticMarkup(<ShotReadinessInspector detail={detail} />);
    expect(html).toContain("七项 Gate");
    expect(html).toContain("角色");
    expect(html).toContain("场景");
    expect(html).toContain("参考");
    expect(html).toContain("提示词");
    expect(html).toContain("工作流");
    expect(html).toContain("输出");
    expect(html).toContain("ComfyUI");
    expect(html).toContain("01234567");
    expect(html).toContain("Project");
    expect(html).toContain("Scene");
    expect(html).toContain("reference-set-1");
    expect(html).toContain("ComfyUI 离线");
    expect(html).toContain("旧上下文");
    expect(html).toContain("keyframe-1");
  });

  it("normalizes missing gates to seven visible incomplete entries and exposes check messages", () => {
    expect(readinessGateEntries([{ key: "CHARACTER", state: "PASS", checks: [] }])).toHaveLength(7);
    expect(readinessGateEntries([]).every((state) => state === "INCOMPLETE")).toBe(true);
    expect(readinessCheckMessages([{ code: "C1", message: "缺少角色" }, { code: "C2", message: "缺少场景" }])).toEqual(["缺少角色", "缺少场景"]);
  });
});
