import { renderToStaticMarkup } from "react-dom/server";
import { act, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import type { AssetView } from "../../types/asset";
import type { ProductionBatchReviewProductivity, ProductionReviewProductivityItem } from "../../services/tauriClient";
import { regenerateProductionItem } from "../../services/tauriClient";
import { ShotBatchReviewBoard, isVideoReviewReworkAvailable, matchesFilter, reviewCounts, reviewImageIdsForItem, toCompareItem, toLocalCompareItem } from "./ShotBatchReviewBoard";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const asset = (id: string, kind: "image" | "video" = "image"): AssetView => ({
  id, assetType: kind, category: kind === "video" ? "generated_video" : "generated_image", name: id, originalName: id,
  mimeType: kind === "video" ? "video/mp4" : "image/png", fileSize: 1, createdAt: "2026-08-27T00:00:00Z", isFavorite: false, tags: [],
});

const reviewItem = (overrides: Partial<ProductionReviewProductivityItem> = {}): ProductionReviewProductivityItem => ({
  itemId: "item-1", ordinal: 0, taskId: "task-1", taskStatus: "SUCCEEDED", productionItemStatus: "SUCCEEDED", reviewStatus: "UNREVIEWED", reviewNote: "",
  preferred: false, workflowVersionId: "workflow-1", recipeId: "recipe-1", qualityProfile: "QUALITY", createdAt: "2026-08-27T00:00:00Z", outputAssets: [asset("asset-a")],
  shotId: "shot-1", stage: "IMAGE", selectedAssetId: undefined, reviewable: true,
  candidateAssets: [{ assetId: "asset-a", assetType: "image", name: "候选 A", mimeType: "image/png", thumbnailAvailable: true, taskId: "task-1", selected: false }],
  context: { shotId: "shot-1", stage: "IMAGE", snapshotAvailable: true, promptText: "prompt", referenceSets: [], referenceAssets: [] },
  ...overrides,
});

const reviewBatch = (items: ProductionReviewProductivityItem[]): ProductionBatchReviewProductivity => ({
  batch: {} as never,
  total: items.length,
  successCount: items.filter((item) => item.productionItemStatus === "SUCCEEDED").length,
  failedCount: items.filter((item) => item.productionItemStatus === "FAILED").length,
  unreviewedCount: items.filter((item) => item.reviewStatus === "UNREVIEWED").length,
  approvedCount: items.filter((item) => item.reviewStatus === "APPROVED").length,
  starredCount: items.filter((item) => item.reviewStatus === "STARRED").length,
  regenerateCount: items.filter((item) => item.reviewStatus === "REGENERATE").length,
  rejectedCount: items.filter((item) => item.reviewStatus === "REJECTED").length,
  items,
});

type TestEvent = {
  type: string;
  bubbles?: boolean;
  cancelable?: boolean;
  target?: TestNode;
  currentTarget?: TestNode;
  key?: string;
  defaultPrevented?: boolean;
  preventDefault: () => void;
  stopPropagation: () => void;
};

type TestListener = (event: TestEvent) => void;

class TestNode {
  readonly nodeType: number = 1;
  readonly childNodes: TestNode[] = [];
  parentNode: TestNode | null = null;
  ownerDocument: TestDocument;
  private readonly listeners = new Map<string, TestListener[]>();

  constructor(ownerDocument: TestDocument) {
    this.ownerDocument = ownerDocument;
  }

  get firstChild(): TestNode | null { return this.childNodes[0] ?? null; }
  get nextSibling(): TestNode | null {
    if (!this.parentNode) return null;
    const index = this.parentNode.childNodes.indexOf(this);
    return index >= 0 ? this.parentNode.childNodes[index + 1] ?? null : null;
  }
  get parentElement(): TestElement | null { return this.parentNode instanceof TestElement ? this.parentNode : null; }
  get textContent(): string { return this.childNodes.map((child) => child.textContent).join(""); }
  set textContent(value: string | null) {
    this.childNodes.splice(0, this.childNodes.length);
    if (value) this.appendChild(new TestTextNode(this.ownerDocument, value));
  }

  appendChild<T extends TestNode>(child: T): T {
    if (child.parentNode) child.parentNode.removeChild(child);
    child.parentNode = this;
    this.childNodes.push(child);
    return child;
  }

  insertBefore<T extends TestNode>(child: T, before: TestNode | null): T {
    if (!before) return this.appendChild(child);
    if (child.parentNode) child.parentNode.removeChild(child);
    const index = this.childNodes.indexOf(before);
    child.parentNode = this;
    this.childNodes.splice(index < 0 ? this.childNodes.length : index, 0, child);
    return child;
  }

  removeChild<T extends TestNode>(child: T): T {
    const index = this.childNodes.indexOf(child);
    if (index >= 0) this.childNodes.splice(index, 1);
    child.parentNode = null;
    return child;
  }

  addEventListener(type: string, listener: TestListener): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener]);
  }

  removeEventListener(type: string, listener: TestListener): void {
    this.listeners.set(type, (this.listeners.get(type) ?? []).filter((candidate) => candidate !== listener));
  }

  dispatchEvent(event: TestEvent): boolean {
    let stopped = false;
    event.target ??= this;
    event.preventDefault = () => { event.defaultPrevented = true; };
    event.stopPropagation = () => { stopped = true; };
    for (const listener of this.listeners.get(event.type) ?? []) {
      event.currentTarget = this;
      listener(event);
    }
    if (!stopped && event.bubbles !== false && this.parentNode) this.parentNode.dispatchEvent(event);
    return !event.defaultPrevented;
  }

  contains(node: TestNode | null): boolean {
    if (!node) return false;
    return node === this || this.childNodes.some((child) => child.contains(node));
  }
}

class TestTextNode extends TestNode {
  readonly nodeType = 3;
  nodeValue: string;

  constructor(ownerDocument: TestDocument, value: string) {
    super(ownerDocument);
    this.nodeValue = value;
  }

  override get textContent(): string { return this.nodeValue; }
  override set textContent(value: string | null) { this.nodeValue = value ?? ""; }
}

class TestElement extends TestNode {
  readonly nodeType = 1;
  readonly tagName: string;
  readonly nodeName: string;
  readonly namespaceURI = "http://www.w3.org/1999/xhtml";
  readonly attributes = new Map<string, string>();
  readonly style = { setProperty: (_name: string, _value: string) => undefined, removeProperty: (_name: string) => undefined };
  className = "";
  disabled = false;
  value = "";
  tabIndex = 0;

  constructor(ownerDocument: TestDocument, tagName: string) {
    super(ownerDocument);
    this.tagName = tagName.toUpperCase();
    this.nodeName = this.tagName;
  }

  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
    if (name === "class") this.className = value;
    if (name === "disabled") this.disabled = true;
  }
  getAttribute(name: string): string | null { return this.attributes.get(name) ?? null; }
  removeAttribute(name: string): void { this.attributes.delete(name); if (name === "disabled") this.disabled = false; }
  hasAttribute(name: string): boolean { return this.attributes.has(name); }
  focus(): void { this.ownerDocument.activeElement = this; }
  blur(): void { if (this.ownerDocument.activeElement === this) this.ownerDocument.activeElement = this.ownerDocument.body; }
  click(): void { this.dispatchEvent({ type: "click", bubbles: true, cancelable: true, target: this, preventDefault: () => undefined, stopPropagation: () => undefined }); }
  getBoundingClientRect(): { top: number; left: number; width: number; height: number } { return { top: 0, left: 0, width: 0, height: 0 }; }
}

class TestDocument extends TestNode {
  readonly nodeType = 9;
  readonly nodeName = "#document";
  readonly documentElement: TestElement;
  readonly head: TestElement;
  readonly body: TestElement;
  activeElement: TestElement;
  defaultView: Record<string, unknown>;

  constructor() {
    super(null as unknown as TestDocument);
    this.ownerDocument = this;
    this.documentElement = new TestElement(this, "html");
    this.head = new TestElement(this, "head");
    this.body = new TestElement(this, "body");
    this.documentElement.appendChild(this.head);
    this.documentElement.appendChild(this.body);
    this.appendChild(this.documentElement);
    this.activeElement = this.body;
    this.defaultView = {};
  }

  createElement(tagName: string): TestElement { return new TestElement(this, tagName); }
  createElementNS(_namespace: string, tagName: string): TestElement { return this.createElement(tagName); }
  createTextNode(value: string): TestTextNode { return new TestTextNode(this, value); }
  createComment(value: string): TestTextNode { return new TestTextNode(this, value); }
}

let currentTestContainer: TestElement | undefined;

function installTestDom(): { document: TestDocument; window: Record<string, unknown> } {
  const document = new TestDocument();
  const window = {
    document,
    navigator: { userAgent: "vitest" },
    HTMLElement: TestElement,
    HTMLIFrameElement: class extends TestElement {},
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    confirm: vi.fn(() => true),
  } as unknown as Record<string, unknown>;
  document.defaultView = window;
  Object.assign(globalThis, { document, window, HTMLElement: TestElement, Element: TestElement, Node: TestNode, Text: TestTextNode, IS_REACT_ACT_ENVIRONMENT: true });
  Object.defineProperty(globalThis, "navigator", { configurable: true, value: window.navigator });
  currentTestContainer = document.createElement("div");
  document.body.appendChild(currentTestContainer);
  return { document, window };
}

function allElements(root: TestNode): TestElement[] {
  return root.childNodes.flatMap((child) => child instanceof TestElement ? [child, ...allElements(child)] : allElements(child));
}

const screen = {
  getByRole(role: string, options: { name?: string | RegExp } = {}): TestElement {
    const elements = allElements(currentTestContainer ?? new TestDocument()).filter((element) => (role === "button" ? element.tagName === "BUTTON" : element.getAttribute("role") === role));
    const found = elements.find((element) => {
      const name = element.getAttribute("aria-label") ?? element.textContent;
      return options.name === undefined || typeof options.name === "string" ? name === options.name : options.name.test(name);
    });
    if (!found) throw new Error(`No ${role} found`);
    return found;
  },
};

const fireEvent = {
  click(element: TestElement): void { if (!element.disabled) element.click(); },
  keyDown(element: TestElement, init: { key: string }): void { element.dispatchEvent({ type: "keydown", key: init.key, bubbles: true, cancelable: true, target: element, preventDefault: () => undefined, stopPropagation: () => undefined }); },
};

async function render(ui: ReactNode): Promise<{ container: TestElement; unmount: () => Promise<void> }> {
  installTestDom();
  const container = currentTestContainer!;
  const root = createRoot(container as unknown as Element);
  await act(async () => { root.render(ui); });
  return { container, unmount: async () => { await act(async () => { root.unmount(); }); } };
}

describe("ShotBatchReviewBoard adapter", () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it("keeps the legacy image/video controls and does not invoke callbacks during render", () => {
    const onSelect = vi.fn();
    const imageShot = { id: "shot-1", ordinal: 0, name: "镜头 1", stageConfigs: [], referenceAssets: [], generationLinks: [{ stage: "image", task: { outputAssetIds: ["asset-a"] } }], status: "READY", imageStatus: "READY", videoStatus: "NOT_STARTED" } as never;
    const videoShot = { id: "shot-2", ordinal: 1, name: "镜头 2", stageConfigs: [], referenceAssets: [], generationLinks: [{ stage: "video", task: { outputAssetIds: ["asset-v"] } }], status: "READY", imageStatus: "READY", videoStatus: "READY" } as never;
    const common = { projectId: "project-1", assets: [asset("asset-a"), asset("asset-v", "video")], busy: false, onAssetsLoaded: vi.fn(), onSelect, onRetry: vi.fn() };
    expect(renderToStaticMarkup(<ShotBatchReviewBoard {...common} shots={[imageShot]} stage="image" />)).toContain("设为关键帧");
    expect(renderToStaticMarkup(<ShotBatchReviewBoard {...common} shots={[videoShot]} stage="video" />)).toContain("设为最终视频");
    expect(renderToStaticMarkup(<ShotBatchReviewBoard {...common} shots={[imageShot]} stage="image" />)).toContain("打开 A/B 对比");
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("builds local A/B items from loaded assets without review status or context", () => {
    const shot = { id: "shot-local", ordinal: 2, name: "本地镜头", stageConfigs: [], referenceAssets: [], generationLinks: [{ stage: "image", task: { outputAssetIds: ["asset-a"] } }] } as never;
    const local = toLocalCompareItem(shot, [asset("asset-a")], "image", "project-1");
    expect(local.candidates).toHaveLength(1);
    expect(local.reviewStatus).toBeUndefined();
    expect(local.context).toBeUndefined();
    expect(local.candidates[0].selected).toBe(false);
  });

  it("maps the enhanced payload without fetching candidate metadata and separates status from Shot selection", () => {
    const item = reviewItem({ selectedAssetId: "asset-a", reviewStatus: "APPROVED" });
    const mapped = toCompareItem(item, "project-1", { "asset-a": "blob:asset-a" });
    expect(mapped.candidates[0]).toMatchObject({ id: "asset-a", imageUrl: "blob:asset-a", selected: false });
    expect(mapped.selectedCandidateId).toBe("asset-a");
    expect(mapped.reviewStatus).toBe("APPROVED");
    expect(mapped.contextSnapshot?.contextHash).toBeUndefined();
  });

  it("keeps compact filter counts bounded to the already-loaded review items", () => {
    const items = [reviewItem(), reviewItem({ itemId: "item-2", reviewStatus: "APPROVED" }), reviewItem({ itemId: "item-3", reviewStatus: "STARRED" }), reviewItem({ itemId: "item-4", reviewStatus: "REGENERATE" }), reviewItem({ itemId: "item-5", reviewStatus: "REJECTED" }), reviewItem({ itemId: "item-6", reviewStatus: "REGENERATE", taskStatus: "FAILED", productionItemStatus: "FAILED" })];
    expect(reviewCounts(items)).toEqual({ unreviewed: 1, approved: 1, starred: 1, regenerate: 2, rejected: 1, failed: 1 });
    expect(matchesFilter(items[0], "UNREVIEWED")).toBe(true);
    expect(matchesFilter(items[1], "APPROVED")).toBe(true);
    expect(matchesFilter(items[2], "STARRED")).toBe(true);
    expect(matchesFilter(items[3], "REGENERATE")).toBe(true);
    expect(matchesFilter(items[4], "REJECTED")).toBe(true);
    expect(matchesFilter(items[5], "FAILED")).toBe(true);
    expect(matchesFilter(items[0], "NEEDS_REVIEW" as never)).toBe(false);
    expect(items.filter((item) => matchesFilter(item, "FAILED"))).toHaveLength(1);
  });

  it("renders every review filter in the real DOM and changes the visible review item", async () => {
    const items = [
      reviewItem({ itemId: "unreviewed", shotId: "shot-unreviewed" }),
      reviewItem({ itemId: "approved", shotId: "shot-approved", reviewStatus: "APPROVED" }),
      reviewItem({ itemId: "starred", shotId: "shot-starred", reviewStatus: "STARRED" }),
      reviewItem({ itemId: "regenerate", shotId: "shot-regenerate", reviewStatus: "REGENERATE" }),
      reviewItem({ itemId: "rejected", shotId: "shot-rejected", reviewStatus: "REJECTED" }),
      reviewItem({ itemId: "failed", shotId: "shot-failed", reviewStatus: "FAILED", taskStatus: "FAILED", productionItemStatus: "FAILED" }),
    ];
    const loader = vi.fn(async () => reviewBatch(items));
    const rendered = await render(<ShotBatchReviewBoard
      projectId="project-1"
      shots={[]}
      assets={[]}
      stage="image"
      busy={false}
      onAssetsLoaded={vi.fn()}
      onSelect={vi.fn()}
      onRetry={vi.fn()}
      reviewBatchId="batch-1"
      reviewBatchLoader={loader}
    />);
    await act(async () => { await Promise.resolve(); });
    for (const label of ["全部 6", "未审核 1", "已通过 1", "标星 1", "待返工 1", "已拒绝 1", "失败 1"]) expect(screen.getByRole("button", { name: label })).toBeDefined();
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "已通过 1" })); });
    expect(rendered.container.textContent).toContain("shot-approved");
    expect(rendered.container.textContent).not.toContain("shot-unreviewed");
    await rendered.unmount();
  });

  it("requires confirmation before creating an eligible video rework batch and never starts the queue", async () => {
    const item = reviewItem({
      itemId: "video-item",
      shotId: "video-shot",
      stage: "VIDEO",
      outputAssets: [asset("video-a", "video")],
      candidateAssets: [{ assetId: "video-a", assetType: "video", name: "视频 A", mimeType: "video/mp4", thumbnailAvailable: false, selected: false }],
    });
    const loader = vi.fn(async () => reviewBatch([item]));
    const onOpenProductionQueue = vi.fn();
    const rendered = await render(<ShotBatchReviewBoard projectId="project-1" shots={[]} assets={[]} stage="video" busy={false} onAssetsLoaded={vi.fn()} onSelect={vi.fn()} onRetry={vi.fn()} reviewBatchId="batch-1" reviewBatchLoader={loader} onOpenProductionQueue={onOpenProductionQueue} />);
    await act(async () => { await Promise.resolve(); });
    const confirm = vi.fn(() => false);
    (globalThis.window as unknown as { confirm: typeof confirm }).confirm = confirm;
    vi.mocked(invoke).mockClear();
    fireEvent.click(screen.getByRole("button", { name: "创建返工批次" }));
    expect(confirm).toHaveBeenCalledOnce();
    expect(invoke).not.toHaveBeenCalledWith("production_item_review_regenerate", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith("production_queue_start", expect.anything());
    expect(onOpenProductionQueue).not.toHaveBeenCalled();
    await rendered.unmount();
  });

  it("creates a confirmed video rework batch with autoStart false, keeps the item selected, and only navigates", async () => {
    const item = reviewItem({
      itemId: "video-item",
      shotId: "video-shot",
      stage: "VIDEO",
      outputAssets: [asset("video-a", "video")],
      candidateAssets: [{ assetId: "video-a", assetType: "video", name: "视频 A", mimeType: "video/mp4", thumbnailAvailable: false, selected: false }],
    });
    const secondItem = { ...item, itemId: "video-item-2", shotId: "video-shot-2" };
    const loader = vi.fn(async () => reviewBatch([item, secondItem]));
    const onOpenProductionQueue = vi.fn();
    const rendered = await render(<ShotBatchReviewBoard projectId="project-1" shots={[]} assets={[]} stage="video" busy={false} onAssetsLoaded={vi.fn()} onSelect={vi.fn()} onRetry={vi.fn()} reviewBatchId="batch-1" reviewBatchLoader={loader} onOpenProductionQueue={onOpenProductionQueue} />);
    await act(async () => { await Promise.resolve(); });
    const confirm = vi.fn(() => true);
    (globalThis.window as unknown as { confirm: typeof confirm }).confirm = confirm;
    vi.mocked(invoke).mockResolvedValue({ selectedCount: 1 });
    fireEvent.click(screen.getByRole("button", { name: "创建返工批次" }));
    await act(async () => { await Promise.resolve(); await Promise.resolve(); });
    expect(confirm).toHaveBeenCalledOnce();
    expect(invoke).toHaveBeenCalledWith("production_item_review_regenerate", { request: expect.objectContaining({ itemId: "video-item", autoStart: false }) });
    expect(invoke).not.toHaveBeenCalledWith("production_queue_start", expect.anything());
    expect(onOpenProductionQueue).toHaveBeenCalledOnce();
    expect(rendered.container.textContent).toContain("video-shot");
    expect(rendered.container.textContent).not.toContain("video-shot-2");
    await rendered.unmount();
  });

  it("keeps image review regeneration unavailable and exposes only the legacy retry path", async () => {
    const item = reviewItem({ itemId: "image-item", shotId: "image-shot", stage: "IMAGE" });
    const loader = vi.fn(async () => reviewBatch([item]));
    const onRetry = vi.fn();
    const rendered = await render(<ShotBatchReviewBoard projectId="project-1" shots={[]} assets={[]} stage="image" busy={false} onAssetsLoaded={vi.fn()} onSelect={vi.fn()} onRetry={onRetry} reviewBatchId="batch-1" reviewBatchLoader={loader} />);
    await act(async () => { await Promise.resolve(); });
    const createButton = screen.getByRole("button", { name: "创建返工批次" });
    expect(isVideoReviewReworkAvailable(item)).toBe(false);
    expect(createButton.disabled).toBe(true);
    vi.mocked(invoke).mockClear();
    fireEvent.click(createButton);
    expect(invoke).not.toHaveBeenCalledWith("production_item_review_regenerate", expect.anything());
    expect(invoke).not.toHaveBeenCalledWith("production_item_review_regenerate_marked", expect.anything());
    expect(onRetry).not.toHaveBeenCalled();
    await rendered.unmount();
  });

  it("limits review image reads to the current item instead of the whole batch", () => {
    const current = reviewItem({ itemId: "current", candidateAssets: [
      { assetId: "current-a", assetType: "image", name: "A", mimeType: "image/png", thumbnailAvailable: true, selected: false },
      { assetId: "current-video", assetType: "video", name: "视频", mimeType: "video/mp4", thumbnailAvailable: false, selected: false },
    ] });
    const other = reviewItem({ itemId: "other", candidateAssets: [{ assetId: "other-a", assetType: "image", name: "其他", mimeType: "image/png", thumbnailAvailable: true, selected: false }] });
    expect(reviewImageIdsForItem(current)).toEqual(["current-a"]);
    expect(reviewImageIdsForItem(other)).toEqual(["other-a"]);
    expect(reviewImageIdsForItem(undefined)).toEqual([]);
  });

  it("forces regeneration payload autoStart false while retaining the wire field", async () => {
    vi.mocked(invoke).mockResolvedValue({});
    await regenerateProductionItem({ projectId: "project-1", batchId: "batch-1", itemId: "item-1", useOriginalSeed: false, autoStart: true });
    expect(invoke).toHaveBeenCalledWith("production_item_review_regenerate", { request: expect.objectContaining({ projectId: "project-1", batchId: "batch-1", itemId: "item-1", autoStart: false }) });
  });
});
