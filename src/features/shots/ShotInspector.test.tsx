import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { AssetView } from "../../types/asset";
import { ShotInspector } from "./ShotInspector";

const reference: AssetView = {
  id: "ref-1",
  assetType: "image",
  category: "source_image",
  name: "character-front",
  originalName: "character-front.png",
  mimeType: "image/png",
  width: 1024,
  height: 1024,
  fileSize: 512,
  createdAt: "2026-08-25T00:00:00Z",
  thumbnailAvailable: true,
  isFavorite: false,
  tags: [],
};

const recipe = {
  workflowId: "h3",
  workflowVersionId: "h3-v1",
  recipeId: "h3-r1",
  name: "H3 Quality",
  category: "video",
  mode: "reference_to_video",
  fields: [
    { key: "width", type: "integer" as const, label: "Width", required: true, default: 1280 },
    { key: "duration", type: "integer" as const, label: "Duration", required: true, default: 8 },
    { key: "steps", type: "integer" as const, label: "Steps", required: true, default: 20 },
    { key: "cfg", type: "number" as const, label: "CFG", required: true, default: 7 },
    { key: "seed", type: "seed" as const, label: "Seed", required: false, defaultMode: "random" as const },
  ],
};

describe("ShotInspector", () => {
  it("renders the primary generate action and collapsed advanced parameters", () => {
    const html = renderToStaticMarkup(
      <ShotInspector
        projectId="project-1"
        stage="video"
        currentRecipe={recipe}
        currentDraft={{ workflowVersionId: "h3-v1", recipeId: "h3-r1", values: { width: { type: "integer", value: 1280 }, duration: { type: "integer", value: 8 }, steps: { type: "integer", value: 20 }, cfg: { type: "number", value: 7 }, seed: { type: "seed_random" } } }}
        onGenerate={vi.fn()}
        configDirty
        onSave={vi.fn()}
      />,
    );

    expect(html).toContain("镜头设置");
    expect(html).toContain("生成");
    expect(html).toContain("保存配置");
    expect(html).toContain("高级设置");
    expect(html).toContain("<details class=\"shot-inspector-advanced\">");
    expect(html).toContain("当前工作流信息");
    expect(html).toContain("1 个参数");
    expect(html.indexOf("<details class=\"shot-inspector-advanced\">")).toBeGreaterThan(html.indexOf(">CFG</span>"));
    expect(html.indexOf("<details class=\"shot-inspector-advanced\">")).toBeGreaterThan(html.indexOf(">时长（秒）</span>"));
  });

  it("renders ordered references, anchor actions, and the keyframe contract without selecting anything", () => {
    const html = renderToStaticMarkup(
      <ShotInspector
        projectId="project-1"
        stage="video"
        onGenerate={vi.fn()}
        activeTab="references"
        references={[{ assetId: "ref-1", asset: reference }, { assetId: "ref-2", label: "scene-wide" }]}
        availableReferences={[reference]}
        referenceAnchors={[{ id: "anchor-1", name: "角色锚点", kind: "CHARACTER", usable: true }]}
        selectedAnchorId="anchor-1"
        keyframeAsset={reference}
        onReferenceMove={vi.fn()}
        onReferenceRemove={vi.fn()}
        onApplyAnchor={vi.fn()}
        onSaveReferences={vi.fn()}
      />,
    );

    expect(html).toContain("有序参考图");
    expect(html).toContain("@图片1");
    expect(html).toContain("@图片2");
    expect(html).toContain("角色锚点");
    expect(html).toContain("追加锚点");
    expect(html).toContain("替换锚点");
    expect(html).toContain("关键帧");
  });

  it("keeps prompt preview, template slot, and apply as callback-only UI", () => {
    const html = renderToStaticMarkup(
      <ShotInspector
        projectId="project-1"
        stage="image"
        onGenerate={vi.fn()}
        activeTab="prompt"
        promptText="wide establishing shot"
        promptPreview="wide establishing shot, frozen"
        promptTemplate={<div>提示词模板插槽</div>}
        promptLibrary={[{ id: "prompt-1", name: "Cinematic", versionCount: 3 }]}
        selectedPromptId="prompt-1"
        onPromptSelect={vi.fn()}
        onLoadPrompt={vi.fn()}
        onPreviewPrompt={vi.fn()}
        onApplyPrompt={vi.fn()}
      />,
    );

    expect(html).toContain("提示词预览");
    expect(html).toContain("wide establishing shot, frozen");
    expect(html).toContain("提示词模板插槽");
    expect(html).toContain("应用提示词");
  });
});
