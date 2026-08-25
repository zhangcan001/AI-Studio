import { describe, expect, it } from "vitest";

const keyUiSources = import.meta.glob([
  "../app/StudioShell.tsx",
  "../components/studio/StudioTopBar.tsx",
  "../components/studio/StudioGlobalRail.tsx",
  "../features/shots/ProjectStructureTree.tsx",
  "../features/shots/ShotCreationWorkspace.tsx",
  "../features/shots/ShotInspector.tsx",
  "../features/shots/ProductionStructurePanel.tsx",
  "../features/shots/ProjectProductionPipeline.tsx",
  "../features/shots/SceneProductionPanel.tsx",
  "../features/shots/EpisodeProductionPanel.tsx",
  "../features/shots/SeriesProductionPanel.tsx",
  "../features/production/ProductionQueueDrawer.tsx",
  "../features/production/ProductionBatchRunbookPanel.tsx",
  "../features/production/ProductionRunPanel.tsx",
  "../features/production/ProductionAuditCenter.tsx",
  "../features/studio/ProductionBatchReviewWorkspace.tsx",
  "../features/studio/ProductionQueuePanel.tsx",
  "../features/assets/AssetVideoBatchWorkspace.tsx",
  "../features/assets/AssetPreview.tsx",
  "../features/experiments/WorkflowBenchmarkPanel.tsx",
  "../features/experiments/ExperimentResultGrid.tsx",
  "../features/shots/ShotBatchPlanner.tsx",
  "../features/prompts/PromptLibraryPanel.tsx",
  "../features/prompts/PromptTemplateVariableHelper.tsx",
  "../features/prompts/promptTemplateState.ts",
  "../features/workflows/WorkflowWorkspace.tsx",
  "../features/workflows/WorkflowImportIssues.tsx",
  "../features/settings/SettingsWorkspace.tsx",
  "../features/studio/DynamicFormRenderer.tsx",
], { query: "?raw", import: "default", eager: true }) as Record<string, string>;

const forbiddenVisibleCopy = [
  "PROJECT STRUCTURE",
  "CREATION CONTEXT",
  "GENERATE",
  "INSPECTOR",
  "RUNTIME",
  "CANDIDATE",
  "SETTINGS",
  "PROMPT PREVIEW",
  "PRODUCTION QUEUE",
  "WORKFLOW / RECIPE",
  "RUN IMAGES",
  "RUN VIDEO",
  "RETRY H3",
  "Production Run",
  "Prompt Library",
  "Asset Library",
  "Episode Production",
  "Scene Production",
  "Series Production",
  "MiniMax",
  "Kera2",
  "Turbo",
  "READY 批次",
  "label: \"Project\"",
  "label: \"Anchor\"",
  "label: \"Custom\"",
  ">Python<",
  "READY ·",
  "WARNING ·",
  "BLOCKED ·",
  "WORKFLOW_UNAVAILABLE",
];

function readKeyUi(): string {
  return Object.values(keyUiSources).join("\n");
}

describe("AI Studio 中文界面守卫", () => {
  it("关键页面不重新出现普通英文界面文案", () => {
    const source = readKeyUi();
    const findings = forbiddenVisibleCopy.filter((copy) => copy === "GENERATE"
      ? /(?<![A-Z])GENERATE(?![A-Z])/.test(source)
      : source.includes(copy));
    expect(findings).toEqual([]);
  });

  it("保留中文导航和生产入口文案", () => {
    const source = readKeyUi();
    expect(source).toContain("项目结构");
    expect(source).toContain("生产队列");
    expect(source).toContain("参数面板");
    expect(source).toContain("提示词");
    expect(source).toContain("参考素材");
  });
});
