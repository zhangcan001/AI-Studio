import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { AssetView } from "../../types/asset";
import { REVIEW_NOTE_MAX_BYTES, validateReviewNote } from "../../types/reviewProductivity";
import type { ReviewCompareCandidate, ReviewCompareItem } from "../../types/reviewProductivity";
import { ReviewCompareWorkspace } from "./ReviewCompareWorkspace";

const asset = (id: string, kind: "image" | "video" = "image"): AssetView => ({
  id,
  assetType: kind,
  category: kind === "video" ? "generated_video" : "generated_image",
  name: id,
  originalName: id,
  mimeType: kind === "video" ? "video/mp4" : "image/png",
  fileSize: 12,
  createdAt: "2026-08-27T00:00:00Z",
  isFavorite: false,
  tags: [],
});

const candidate = (id: string, overrides: Partial<ReviewCompareCandidate> = {}): ReviewCompareCandidate => ({
  id,
  asset: asset(id),
  imageUrl: `blob:${id}`,
  ...overrides,
});

const context = {
  source: "snapshot" as const,
  historicalName: "历史镜头名",
  currentName: "当前镜头名",
  prompt: "雨夜巷口",
  context: "scene context",
  workflowName: "H3 Workflow",
  recipeName: "I2V Recipe",
  contextHash: "sha256:abc",
  referenceSets: [{ id: "set-1", name: "主角参考集", assets: [] }],
  referenceAssets: [{ id: "ref-1", name: "主角正面" }],
  outputSpec: { width: 1280, height: 720 },
  stageInput: "关键帧 A",
  readiness: { status: "READY" },
};

const item = (overrides: Partial<ReviewCompareItem> = {}): ReviewCompareItem => ({
  id: "shot-1",
  ordinal: 0,
  shotId: "shot-1",
  shotName: "Shot 01",
  candidates: [candidate("a", { label: "候选 A", context }), candidate("b", { label: "候选 B", context })],
  reviewNote: "",
  ...overrides,
});

describe("ReviewCompareWorkspace", () => {
  it("renders two candidates in independent A/B image previews and exposes swap/slot controls", () => {
    const html = renderToStaticMarkup(<ReviewCompareWorkspace items={[item()]} onApprove={vi.fn()} />);

    expect(html).toContain('data-slot="A"');
    expect(html).toContain('data-slot="B"');
    expect(html).toContain("候选 A");
    expect(html).toContain("候选 B");
    expect(html).toContain("交换 A/B");
    expect(html.match(/class="zoomable-image-preview /g)).toHaveLength(2);
    expect(html).toContain("将 候选 A 放入 A 槽位");
    expect(html).toContain("将 候选 B 放入 B 槽位");
  });

  it("renders one candidate in A only and keeps candidate clicks presentation-only", () => {
    const onApprove = vi.fn();
    const onReject = vi.fn();
    const html = renderToStaticMarkup(<ReviewCompareWorkspace items={[item({ candidates: [candidate("only", { label: "唯一候选" })] })]} onApprove={onApprove} onReject={onReject} />);

    expect(html).toContain('data-slot="A"');
    expect(html).not.toContain('data-slot="B"');
    expect(html).not.toContain("交换 A/B");
    expect(html).toContain("唯一候选");
    expect(onApprove).not.toHaveBeenCalled();
    expect(onReject).not.toHaveBeenCalled();
  });

  it("renders previous/next navigation without invoking mutation callbacks", () => {
    const onConfirmAndApprove = vi.fn();
    const onRegenerate = vi.fn();
    const onItemChange = vi.fn();
    const html = renderToStaticMarkup(<ReviewCompareWorkspace items={[item(), item({ id: "shot-2", ordinal: 1, shotName: "Shot 02" })]} onItemChange={onItemChange} onConfirmAndApprove={onConfirmAndApprove} onRegenerate={onRegenerate} />);

    expect(html).toContain("上一项");
    expect(html).toContain("下一项");
    expect(html).toContain("1 / 2");
    expect(html).toContain("ArrowLeft ArrowRight 1 2");
    expect(onItemChange).not.toHaveBeenCalled();
    expect(onConfirmAndApprove).not.toHaveBeenCalled();
    expect(onRegenerate).not.toHaveBeenCalled();
  });

  it("renders explicit approve, star, reject, regenerate, batch, and note callbacks as actions", () => {
    const html = renderToStaticMarkup(
      <ReviewCompareWorkspace
        items={[item()]}
        onConfirmAndApprove={vi.fn()}
        onApprove={vi.fn()}
        onStar={vi.fn()}
        onReject={vi.fn()}
        onRegenerate={vi.fn()}
        onCreateReworkBatch={vi.fn()}
        onSaveNote={vi.fn()}
      />,
    );

    for (const label of ["确认并通过", "仅通过", "标星", "拒绝", "标记返工", "创建返工批次", "保存备注"]) expect(html).toContain(label);
    expect(html).toContain("确认并通过");
  });

  it("prefers historical snapshot context and renders prompt, refs, and context hash", () => {
    const html = renderToStaticMarkup(<ReviewCompareWorkspace items={[item()]} />);

    expect(html).toContain("历史快照");
    expect(html).toContain("历史镜头名");
    expect(html).toContain("雨夜巷口");
    expect(html).toContain("sha256:abc");
    expect(html).toContain("主角参考集");
    expect(html).toContain("主角正面");
    expect(html).toContain("当前 Workflow 未提供独立 Negative Prompt 输入");
  });

  it("renders the exact legacy message and does not invent a historical name", () => {
    const html = renderToStaticMarkup(<ReviewCompareWorkspace items={[item({ name: "当前名称", context: undefined, historicalContext: undefined, contextSnapshot: undefined, snapshot: undefined, candidates: [candidate("only", { context: undefined })] })]} />);

    expect(html).toContain("旧版任务，无生产准备快照");
    expect(html).toContain("当前名称");
    expect(html).not.toContain("历史名称");
  });

  it("uses native metadata-preloaded video elements for video comparison", () => {
    const html = renderToStaticMarkup(<ReviewCompareWorkspace items={[item({ candidates: [candidate("video-a", { mediaKind: "video", mediaUrl: "/media/a.mp4" }), candidate("video-b", { mediaKind: "video", mediaUrl: "/media/b.mp4" })] })]} />);

    expect(html.match(/<video/g)).toHaveLength(2);
    expect(html).toContain('preload="metadata"');
    expect(html).toContain("playsInline");
    expect(html).not.toContain("asset_read_image");
  });

  it("does not expose set-final-result for failed or non-successful items", () => {
    const html = renderToStaticMarkup(<ReviewCompareWorkspace items={[item({ candidates: [candidate("failed", { productionItemStatus: "FAILED" })] })]} onConfirmAndApprove={vi.fn()} />);

    expect(html).not.toContain("确认并通过");
    expect(html).toContain("仅通过");
    expect(html).toContain('disabled=""');
  });

  it("does not expose final-result selection when the review item has no Shot link", () => {
    const html = renderToStaticMarkup(<ReviewCompareWorkspace items={[item({ shotId: undefined })]} onConfirmAndApprove={vi.fn()} onApprove={vi.fn()} onStar={vi.fn()} onReject={vi.fn()} onRegenerate={vi.fn()} />);

    expect(html).not.toContain("确认并通过");
    expect(html).toContain("仅通过");
    expect(html).toContain("标星");
    expect(html).toContain("拒绝");
    expect(html).toContain("标记返工");
  });

  it("enforces a 4 KiB note limit", () => {
    expect(validateReviewNote("a".repeat(REVIEW_NOTE_MAX_BYTES))).toBeUndefined();
    expect(validateReviewNote("a".repeat(REVIEW_NOTE_MAX_BYTES + 1))).toContain("4 KiB");
    expect(validateReviewNote("界".repeat(REVIEW_NOTE_MAX_BYTES))).toContain("4 KiB");
  });
});
