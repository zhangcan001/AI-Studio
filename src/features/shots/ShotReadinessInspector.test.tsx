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
    for (const label of ["角色", "场景", "参考", "提示词", "工作流", "输出", "ComfyUI"]) {
      expect(html).toContain(label);
    }
    expect(html).toContain("通过");
    expect(html).toContain("阻塞");
    expect(html).toContain("ComfyUI");
    expect(html).toContain("01234567");
    expect(html).toContain("0123456789abcdef0123456789abcdef");
    expect(html).toContain("Project");
    expect(html).toContain("Scene");
    expect(html).toContain("character-1");
    expect(html).toContain("scene-profile-1");
    expect(html).toContain("reference-set-1");
    expect(html).toContain("1 个素材");
    expect(html).toContain("ComfyUI 离线");
    expect(html).toContain("keyframe-1");
  });

  it("shows the already-prepared and stale states without hiding the frozen context evidence", () => {
    const html = renderToStaticMarkup(
      <ShotReadinessInspector
        detail={{
          ...detail,
          alreadyPrepared: true,
          matchingPreparedBatchIds: ["batch-current"],
          stalePreparedBatchIds: ["batch-old"],
          snapshotIdentity: {
            snapshotId: "snapshot-1",
            productionBatchId: "batch-current",
            productionBatchItemId: "item-1",
            contextHash: detail.contextHash,
          },
        }}
      />,
    );

    expect(html).toContain("已准备");
    expect(html).toContain("旧上下文");
    expect(html).toContain("0123456789abcdef0123456789abcdef");
  });

  it("keeps legacy shots visible when no new Profile or ReferenceSet exists", () => {
    const html = renderToStaticMarkup(
      <ShotReadinessInspector
        detail={{
          ...detail,
          stage: "image",
          readiness: { ...detail.readiness, status: "READY", gates: [] },
          resolvedContext: {
            stage: "image",
            profiles: { characters: [], scene: null, props: [], style: null },
            referencePack: { referenceSets: [], referenceAssets: [] },
            legacy: { hasReferencePack: false, usesLegacyShotReferences: true, prompt: "旧版镜头提示词" },
          },
          contextHash: "legacy-context-hash",
          currentStageStatus: "未开始",
          matchingPreparedBatchIds: [],
          stalePreparedBatchIds: [],
          alreadyPrepared: false,
          blockers: [],
          warnings: [],
        }}
      />,
    );

    expect(html).toContain("Legacy Shot");
    expect(html).toContain("沿用旧 Shot prompt / stage config / reference 关系");
    expect(html).toContain("无新 Profile（可能使用 Legacy）");
    expect(html).toContain("无 ReferenceSet");
  });

  it("normalizes missing gates to seven visible incomplete entries and exposes check messages", () => {
    expect(readinessGateEntries([{ key: "CHARACTER", state: "PASS", checks: [] }])).toHaveLength(7);
    expect(readinessGateEntries([]).every((state) => state === "INCOMPLETE")).toBe(true);
    expect(readinessCheckMessages([{ code: "C1", message: "缺少角色" }, { code: "C2", message: "缺少场景" }])).toEqual(["缺少角色", "缺少场景"]);
  });
});
