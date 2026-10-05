// @vitest-environment jsdom
import { StrictMode, useState } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
// @ts-expect-error Node helpers execute only in Vitest; application excludes Node types.
import { readFileSync, readdirSync } from "node:fs";
import { CreatePage } from "./CreatePage";
import { useStudioStore } from "../../stores/studioStore";
import type { CreationContext, GeneratorOption, ProductRun, CreationAsset } from "../../product/types";
import type { AppRoute } from "../../app/routes/types";
import { normalCreate, mediaValue, modeLabel } from "./createModel";
import { fromLegacyLocation, toLegacyLocation } from "../../app/routes/legacyAdapter";
import { resolveCreateReadinessAction } from "./createReadinessAction";
const api = vi.hoisted(() => ({ get: vi.fn(), generatorsList: vi.fn(), createShot: vi.fn(), referencesSet: vi.fn(), selectResult: vi.fn(), readinessGet: vi.fn(), generate: vi.fn(), runGet: vi.fn(), retry: vi.fn(), libraryGet: vi.fn(), bindingSet: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { creation: { ...api, mediaUrl: (_p: string, _id: string) => "http://fixture.invalid/video" }, run: { get: api.runGet, retry: api.retry }, library: {get: api.libraryGet}, project: { generatorBindingSet: api.bindingSet } } }));
const prompt = { type: "textarea", key: "prompt", label: "Prompt", required: true, default: "" } as const;
const negative = { ...prompt, key: "negative_prompt", label: "Negative prompt", required: false };
const params = [{ type: "integer", key: "width", label: "Width", required: true, default: 640, min: 64, max: 1920, step: 8 }, { type: "integer", key: "height", label: "Height", required: true, default: 480 }, { type: "integer", key: "duration_seconds", label: "Duration", required: true, default: 5, min: 1, max: 10 }, { type: "number", key: "cfg", label: "CFG", required: false, default: 1.5 }, { type: "seed", key: "seed", label: "Seed", defaultMode: "random" }] satisfies GeneratorOption["fields"];
function option(ref: string, fields: GeneratorOption["fields"], mediaKind: "image" | "video" = "video"): GeneratorOption { return { selectionRef: ref, name: mediaKind === "image" ? "wfl_krea2_fixture" : "wfl_minimax_h3_fixture", version: "1", mode: ref, mediaKind, availability: true, availabilityReason: null, recommended: true, fields }; }
const image = option("image-opaque", [prompt, negative, ...params.filter(f => f.key !== "duration_seconds")], "image");
const videos = [option("t2v-opaque", [prompt, ...params]), option("i2v-opaque", [prompt, ...params, { type: "image", key: "first_frame", label: "First frame", required: true }]), option("fl-opaque", [prompt, ...params, { type: "image", key: "first_frame", label: "First", required: true }, { type: "image", key: "last_frame", label: "Last", required: true }]), option("ref-opaque", [prompt, ...params, { type: "video", key: "reference_video", label: "Reference video", required: true }])];
const inputs: CreationAsset[] = [{ id: "img1", name: "首帧素材", mediaKind: "image", selected: false }, { id: "img2", name: "尾帧素材", mediaKind: "image", selected: false }, { id: "vid1", name: "参考视频素材", mediaKind: "video", selected: false }];
let candidates: CreationAsset[];
function context(stage: "image" | "video", shotId: string | null = "shot1"): CreationContext { return { projectId: "project", projectName: "Project", stage, shots: [{ id: "shot1", name: "镜头一", ordinal: 0 }, { id: "shot2", name: "镜头二", ordinal: 1 }], selectedShot: shotId ? { summary: { id: shotId, name: "镜头", ordinal: 0 }, prompt: `${stage} prompt`, selectionRef: null, values: {}, referenceAssetIds: [], selectedResultId: null, recentRun: null } : null, candidates: [...candidates], mediaInputs: inputs, promptChoices: [{ promptId: "prm-choice", promptVersionId: "prv-choice", name: "我的提示词", text: "chosen prompt", version: 2 }] }; }
function run(status: ProductRun["status"] = "QUEUED", actions: string[] = []): ProductRun { return { ref: { source: "queue-batch", id: "run" }, projectId: "project", title: "run", status, phase: status, createdAt: "", updatedAt: "", progress: { total: 1, succeeded: status === "SUCCEEDED" ? 1 : 0, failed: status === "FAILED" ? 1 : 0, cancelled: 0 }, recoverability: { retryItemIds: ["old-item"], reviewRequired: 0 }, resultsSummary: [], errorSummary: null, preferredParent: null, availableActions: actions }; }
const navigations = vi.fn(); const dirty = vi.fn();
function Host({ initialStage = "image", initialShot = "shot1" }: { initialStage?: "image" | "video"; initialShot?: string }) {
  const [route, setRoute] = useState<AppRoute>({ kind: "create", projectId: "project", shotId: initialShot || undefined, stage: initialStage });
  if (route.kind !== "create") return <p>离开创作</p>;
  return <CreatePage route={route} onDirtyChange={dirty} navigate={next => { navigations(next); setRoute(next); }} />;
}
beforeEach(() => {
  vi.clearAllMocks(); useStudioStore.getState().resetDraft(); candidates = [];
  api.get.mockImplementation((_p: string, shot: string | null, stage: "image" | "video") => Promise.resolve(context(stage, shot)));
  api.generatorsList.mockImplementation((_p: string, stage: string) => Promise.resolve(stage === "image" ? [image] : videos));
  api.readinessGet.mockResolvedValue({ ready: true, issues: [], fieldErrors: [], actions: [] });
  api.generate.mockResolvedValue({ accepted: true, runRef: { source: "queue-batch", id: "run" }, startOutcome: "STARTED", startIssue: null });
  api.runGet.mockResolvedValue(run()); api.retry.mockResolvedValue(run("RUNNING")); api.createShot.mockResolvedValue({ id: "shot1", name: "镜头一", ordinal: 0 });
  api.referencesSet.mockResolvedValue(undefined); api.selectResult.mockImplementation(async (_p: string, _s: string, _stage: string, id: string) => { candidates = candidates.map(item => ({ ...item, selected: item.id === id })); });
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  URL.createObjectURL = vi.fn(() => "blob:fixture"); URL.revokeObjectURL = vi.fn();
});
describe("M1-1 readiness actions", () => {
  const blocked = (action: string, field?: string) => ({ ready: false, issues: [{ code: "RUNTIME_BLOCKED", message: "private backend detail", details: { action, field, retryable: false, technicalDetails: "private path" } }], fieldErrors: [], actions: [action] });
  async function show(action: string, field?: string) {
    api.readinessGet.mockResolvedValue(blocked(action, field));
    render(<Host />); await loaded();
  }
  it("focuses the generator without changing selection", async () => {
    await show("SELECT_GENERATOR");
    fireEvent.click(await screen.findByRole("button", { name: "重新选择生成器" }));
    expect(document.activeElement).toBe(screen.getByLabelText("选择生成器"));
    expect((screen.getByLabelText("选择生成器") as HTMLSelectElement).value).toBe("image-opaque");
    expect(api.generate).not.toHaveBeenCalled(); expect(navigations).not.toHaveBeenCalled();
  });
  it.each([undefined, "future_field"])("falls back to the actual input region for field %s", async field => {
    await show("EDIT_INPUT", field);
    fireEvent.click(await screen.findByRole("button", { name: field ? "修改此输入" : "检查输入" }));
    expect(document.activeElement).toBe(screen.getByRole("main", { name: "创作输入" }));
    expect(api.generate).not.toHaveBeenCalled();
    expect(screen.queryByText("private backend detail")).toBeNull(); expect(screen.queryByText("private path")).toBeNull();
  });
  it("rechecks with a read-only key without navigation or generation", async () => {
    await show("TRY_LATER");
    const button = await screen.findByRole("button", { name: "重新检查" });
    const before = api.readinessGet.mock.calls.length;
    api.readinessGet.mockResolvedValue({ ready: true, issues: [], fieldErrors: [], actions: [] });
    fireEvent.click(button);
    await screen.findByText("可以生成");
    expect(api.readinessGet.mock.calls.length).toBe(before + 1);
    expect(api.readinessGet).toHaveBeenLastCalledWith(expect.objectContaining({ submissionIdempotencyKey: "readiness-only", projectId: "project" }));
    expect(api.generate).not.toHaveBeenCalled(); expect(api.retry).not.toHaveBeenCalled(); expect(navigations).not.toHaveBeenCalled();
  });
  it("does not guess unknown actions or binding/run destinations", async () => {
    await show("FUTURE_ACTION");
    expect(await screen.findByText("无法自动定位，请根据提示检查当前输入或运行环境。")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "检查运行环境" })).toBeNull();
    expect(api.generate).not.toHaveBeenCalled(); expect(api.retry).not.toHaveBeenCalled(); expect(navigations).not.toHaveBeenCalled();
    expect(resolveCreateReadinessAction({ action: "OPEN_RUN", retryable: false }).kind).toBe("none");
    expect(resolveCreateReadinessAction({ action: "REVIEW_CURRENT_BINDING", retryable: false }).kind).toBe("none");
  });
  it("uses the existing project-list destination", async () => {
    await show("OPEN_PROJECTS");
    fireEvent.click(await screen.findByRole("button", { name: "返回项目列表" }));
    expect(navigations).toHaveBeenLastCalledWith({ kind: "project-list" }); expect(api.generate).not.toHaveBeenCalled();
  });
  it("preserves draft, exact generator, RunRef and accepted state across Settings return", async () => {
    const route = { kind: "create", projectId: "project", shotId: "shot1", stage: "video" } as const;
    const navigate = vi.fn();
    api.generate.mockResolvedValue({ accepted: true, runRef: { source: "queue-batch", id: "run" }, startOutcome: "FAILED_TO_START", startIssue: null });
    const view = render(<CreatePage route={route} navigate={navigate} />); await loaded();
    fireEvent.change(screen.getByLabelText("选择生成器"), { target: { value: "fl-opaque" } });
    fireEvent.change(screen.getByLabelText("提示词"), { target: { value: "Settings return draft" } });
    fireEvent.change(screen.getByLabelText("首帧"), { target: { value: "img1" } });
    fireEvent.change(screen.getByLabelText("尾帧"), { target: { value: "img2" } });
    fireEvent.change(screen.getByLabelText("宽度"), { target: { value: "1280" } });
    fireEvent.click(screen.getByRole("button", { name: "生成" }));
    await screen.findByText("已加入队列，启动失败");
    // Preserve accepted independently of dirty; editing removes accepted by existing contract.
    useStudioStore.getState().setValue("width", { type: "integer", value: 1280 });
    api.readinessGet.mockResolvedValue(blocked("OPEN_RUNTIME_SETTINGS"));
    fireEvent.click(screen.getByRole("button", { name: "生成" }));
    fireEvent.click(await screen.findByRole("button", { name: "检查运行环境" }));
    expect(navigate).toHaveBeenLastCalledWith({ kind: "system-settings", section: "general", returnTo: route });
    const draft = useStudioStore.getState().values;
    view.unmount();
    expect(useStudioStore.getState().draftDirty).toBe(true);
    expect(useStudioStore.getState().creationLabReturn?.accepted?.startOutcome).toBe("FAILED_TO_START");
    render(<CreatePage route={route} navigate={navigate} />); await loaded();
    expect(useStudioStore.getState().values).toEqual(draft);
    expect(useStudioStore.getState().draftDirty).toBe(true);
    expect((screen.getByLabelText("选择生成器") as HTMLSelectElement).value).toBe("fl-opaque");
    expect((screen.getByLabelText("提示词") as HTMLTextAreaElement).value).toBe("Settings return draft");
    await screen.findByText("已加入队列，启动失败");
    fireEvent.click(screen.getByRole("button", { name: "查看运行详情" }));
    expect(navigate).toHaveBeenLastCalledWith({ kind: "runs", projectId: "project", run: { source: "queue-batch", id: "run" } });
    expect(api.generate).toHaveBeenCalledTimes(1);
  });
  it.each(["resolve", "reject"])("ignores a manual recheck %s after switching project", async outcome => {
    let resolve!: (value: unknown) => void; let reject!: (error: unknown) => void;
    const a = { kind: "create", projectId: "project", shotId: "shot1", stage: "image" } as const;
    api.readinessGet.mockResolvedValue(blocked("TRY_LATER"));
    const view = render(<CreatePage route={a} navigate={navigations} />); await loaded();
    const button = await screen.findByRole("button", { name: "重新检查" });
    api.readinessGet.mockImplementationOnce(() => new Promise((yes, no) => { resolve = yes; reject = no; }));
    fireEvent.click(button);
    api.readinessGet.mockResolvedValue({ ready: true, issues: [], fieldErrors: [], actions: [] });
    view.rerender(<CreatePage route={{ ...a, projectId: "other" }} navigate={navigations} />);
    await loaded(); await screen.findByText("可以生成");
    fireEvent.change(screen.getByLabelText("提示词"), { target: { value: "B draft" } });
    await act(async () => { if (outcome === "resolve") resolve(blocked("OPEN_RUNTIME_SETTINGS")); else reject({ code: "ASSET_UNAVAILABLE" }); });
    expect(screen.queryByRole("button", { name: "检查运行环境" })).toBeNull();
    expect(screen.queryByRole("alert")).toBeNull();
    expect(useStudioStore.getState().values.prompt).toEqual({ type: "string", value: "B draft" });
    expect(api.generate).not.toHaveBeenCalled();
  });
});
afterEach(cleanup);
const loaded = () => screen.findByLabelText("选择生成器");
describe("Target11 controller", () => {
  it("uses canonical routes and restores each stage draft", async () => {
    render(<StrictMode><Host /></StrictMode>); await loaded();
    fireEvent.change(screen.getByLabelText("提示词"), { target: { value: "image edit" } });
    fireEvent.click(screen.getByRole("button", { name: "视频" })); await waitFor(() => expect(screen.getByLabelText("提示词").getAttribute("rows")).toBe("5"));
    await waitFor(() => expect((screen.getByLabelText("提示词") as HTMLTextAreaElement).value).toBe("video prompt"));
    fireEvent.change(screen.getByLabelText("提示词"), { target: { value: "video edit" } });
    fireEvent.click(screen.getByRole("button", { name: "图片" })); await loaded();
    await waitFor(() => expect((screen.getByLabelText("提示词") as HTMLTextAreaElement).value).toBe("image edit"));
    fireEvent.click(screen.getByRole("button", { name: "视频" })); await loaded();
    await waitFor(() => expect((screen.getByLabelText("提示词") as HTMLTextAreaElement).value).toBe("video edit"));
    expect(navigations.mock.calls.every(([r]) => r.kind === "create" && r.shotId === "shot1")).toBe(true); expect(dirty).toHaveBeenCalledWith(true);
    fireEvent.change(screen.getByLabelText("镜头"), { target: { value: "shot2" } }); await loaded(); expect(navigations).toHaveBeenLastCalledWith(expect.objectContaining({ shotId: "shot2", kind: "create" }));
  });
  it("has one empty-project primary CTA and navigates newly created Shot", async () => {
    api.get.mockResolvedValueOnce({ ...context("image", null), shots: [] }); render(<Host initialShot="" />);
    fireEvent.click(await screen.findByRole("button", { name: "创建第一个镜头" }));
    await waitFor(() => expect(navigations).toHaveBeenCalledWith(expect.objectContaining({ kind: "create", shotId: "shot1" })));
  });
});
describe("Target12 media", () => {
  it("retains Library intent across StrictMode and consumes only a compatible draft slot", async () => {
    useStudioStore.getState().setPendingAssetIntent({ projectId: "project", assetId: "vid1", assetType: "video" });
    render(<StrictMode><Host initialStage="video" /></StrictMode>); await loaded();
    expect(useStudioStore.getState().pendingAssetIntent?.assetId).toBe("vid1");
    fireEvent.change(screen.getByLabelText("选择生成器"), { target: { value: "ref-opaque" } });
    await waitFor(() => expect(useStudioStore.getState().values.reference_video).toEqual({ type: "video_asset", assetId: "vid1" }));
    expect(useStudioStore.getState().pendingAssetIntent).toBeUndefined(); expect(api.referencesSet).not.toHaveBeenCalled();
  });
  it("maps first/last by schema identity and video as draft-only input", async () => {
    render(<Host initialStage="video" />); await loaded();
    fireEvent.change(screen.getByLabelText("选择生成器"), { target: { value: "fl-opaque" } });
    fireEvent.change(screen.getByLabelText("首帧"), { target: { value: "img2" } }); fireEvent.change(screen.getByLabelText("尾帧"), { target: { value: "img1" } });
    expect(useStudioStore.getState().values.first_frame).toEqual({ type: "image_asset", assetId: "img2" }); expect(useStudioStore.getState().values.last_frame).toEqual({ type: "image_asset", assetId: "img1" });
    fireEvent.change(screen.getByLabelText("选择生成器"), { target: { value: "ref-opaque" } }); fireEvent.change(screen.getByLabelText("参考视频"), { target: { value: "vid1" } });
    expect(within(screen.getByLabelText("参考视频")).queryByText("首帧素材")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "生成" }));
    await waitFor(() => expect(api.generate).toHaveBeenCalled()); expect(api.generate.mock.calls[0][0].values.reference_video).toEqual({ type: "video_asset", assetId: "vid1" }); expect(api.referencesSet).not.toHaveBeenCalled();
    expect(mediaValue({ type: "videos", key: "references", label: "Videos", required: true, minItems: 1, maxItems: 3 }, ["v2", "v1"])).toEqual({ type: "video_assets", assetIds: ["v2", "v1"] });
  });
  it("persistent references use only image choices and keep backend scope checks", async () => {
    render(<Host />); await loaded(); fireEvent.click(screen.getByText("镜头长期参考图"));
    fireEvent.click(screen.getByRole("checkbox", { name: "首帧素材" })); fireEvent.click(screen.getByRole("button", { name: "保存长期参考图" }));
    await waitFor(() => expect(api.referencesSet).toHaveBeenCalledWith("project", "shot1", "image", ["img1"]));
    expect(screen.queryByRole("checkbox", { name: "参考视频素材" })).toBeNull();
  });
});
describe("Target13 inputs", () => {
  it("edits prompts, uses inline library, and presents friendly schema controls", async () => {
    render(<Host />); await loaded(); expect(screen.getByLabelText("负向提示词")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "选择提示词" })); fireEvent.click(screen.getByRole("button", { name: "我的提示词 · 版本 2" }));
    expect((screen.getByLabelText("提示词") as HTMLTextAreaElement).value).toBe("chosen prompt");
    expect(screen.queryByText(/wfl_/)).toBeNull(); expect(api.bindingSet).not.toHaveBeenCalled();
    fireEvent.change(screen.getByLabelText("宽度"), { target: { value: "1280" } }); expect(useStudioStore.getState().values.width).toEqual({ type: "integer", value: 1280 });
    fireEvent.click(screen.getByRole("button", { name: "视频" })); await loaded(); await waitFor(() => expect(screen.queryByLabelText("负向提示词")).toBeNull());
    expect(screen.getByLabelText("时长（秒）").getAttribute("max")).toBe("10"); expect(videos.map(modeLabel)).toEqual(["文生视频", "图生视频", "首尾帧", "参考视频"]);
  });
});
describe("Target14 run", () => {
  it("preserves failed-start RunRef and accepted is not success", async () => {
    api.generate.mockResolvedValue({ accepted: true, runRef: { source: "queue-batch", id: "run" }, startOutcome: "FAILED_TO_START", startIssue: { code: "RUNTIME_BLOCKED" } });
    render(<Host />); await loaded(); fireEvent.click(screen.getByRole("button", { name: "生成" }));
    expect(await screen.findByText("已加入队列，启动失败")).toBeTruthy(); expect(screen.queryByText("已完成")).toBeNull();
    expect(await screen.findByText("排队中")).toBeTruthy(); fireEvent.click(screen.getByRole("button", { name: "查看运行详情" }));
    expect(navigations).toHaveBeenLastCalledWith({ kind: "runs", projectId: "project", run: { source: "queue-batch", id: "run" } });
  });
  it("field errors focus input, and retry only uses ProductRun actions/snapshot locator", async () => {
    api.readinessGet.mockResolvedValue({ ready: false, issues: [{ code: "INPUT_OUT_OF_RANGE", message: "secret", details: { field: "width", action: "EDIT_INPUT", retryable: false } }], fieldErrors: [], actions: ["EDIT_INPUT"] });
    render(<Host />); await loaded(); fireEvent.click(screen.getByRole("button", { name: "生成" }));
    fireEvent.click(await screen.findByRole("button", { name: "修改此输入" })); expect(document.activeElement).toBe(screen.getByLabelText("宽度")); expect(api.generate).not.toHaveBeenCalled(); expect(screen.queryByText("secret")).toBeNull();
  });
  it("shows retry only when allowed, preserving the old run locator", async () => {
    api.runGet.mockResolvedValue(run("FAILED", ["RETRY"])); render(<Host />); await loaded(); fireEvent.click(screen.getByRole("button", { name: "生成" }));
    fireEvent.click(await screen.findByRole("button", { name: "重试原运行" })); await waitFor(() => expect(api.retry).toHaveBeenCalledWith("project", { ref: { source: "queue-batch", id: "run" }, selectedItemIds: ["old-item"] }));
  });
});
describe("Target15 candidates", () => {
  it("displays only stage candidates; selection keeps other candidates and separate review", async () => {
    candidates = [{ id: "c1", name: "候选一", mediaKind: "image", selected: false }, { id: "c2", name: "候选二", mediaKind: "image", selected: false }, { id: "old-video", name: "不属于当前阶段", mediaKind: "video", selected: false }];
    render(<Host />); await loaded(); expect(screen.queryByText("不属于当前阶段")).toBeNull(); fireEvent.click(screen.getAllByRole("button", { name: "选用此结果" })[1]);
    await waitFor(() => expect(api.selectResult).toHaveBeenCalledWith("project", "shot1", "image", "c2")); expect(await screen.findByText("已选用")).toBeTruthy(); expect(screen.getAllByText("候选一").length).toBeGreaterThan(0); expect(screen.getByText(/选用不等于审核/)).toBeTruthy();
  });
});
describe("Target16 architecture", () => {
  it("converges normal entry and preserves explicit advanced paths without a new owner", async () => {
    expect(normalCreate({ kind: "create", projectId: "p", stage: "image" })).toBe(true);
    expect(normalCreate({ kind: "create", projectId: "p", stage: "video", surface: "batch" })).toBe(false);
    expect(normalCreate({ kind: "create", projectId: "p", stage: "video" })).toBe(true);
    const app = readFileSync("src/app/App.tsx", "utf8"); // Phase10: retain the normal-route proof at its actual composition owner.
    const pages = readFileSync("src/app/NormalProductPages.tsx", "utf8");
    expect(app).toContain("<NormalProductPages project={activeProject} route={route} navigate={navigate} onDirtyChange={setShotDraftDirty} />");
    expect(pages).toContain('normalCreate(route) && route.kind === "create"');
    for (const path of readdirSync("src/features/create").filter((path: string) => /\.(tsx?|css)$/.test(path) && !path.includes("test"))) {
      const source = readFileSync(`src/features/create/${path}`, "utf8"); expect(source).not.toMatch(/tauriClient|@tauri-apps|services\/ipc|create\s*\(.*=>/); expect(source).not.toMatch(/<ShotWorkspace|<GenerationStudio|<DirectGenerationEntry/);
    }
    render(<Host />); await loaded(); fireEvent.click(screen.getByText("高级")); fireEvent.click(screen.getByRole("button", { name: "批量生成 / Experiment" })); expect(navigations).toHaveBeenLastCalledWith(expect.objectContaining({ surface: "batch", kind: "create" }));
  }, 15000);
});
it("phase6_target4 canonical Lab routes and legacy deep links preserve Create draft on return", async () => {
  const route = { kind: "create", projectId: "project", shotId: "shot1", stage: "image" } as const;
  const navigate = vi.fn();
  const view = render(<CreatePage route={route} navigate={navigate} />); await loaded();
  fireEvent.change(screen.getByLabelText("提示词"), { target: { value: "Lab return draft" } });
  fireEvent.click(screen.getByRole("button", { name: "工作流 / Benchmark" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "system-settings", section: "advanced-workflows", returnTo: route });
  view.unmount();
  expect(useStudioStore.getState().values.prompt).toEqual({ type: "string", value: "Lab return draft" });
  render(<CreatePage route={route} navigate={navigate} />); await loaded();
  expect((screen.getByLabelText("提示词") as HTMLTextAreaElement).value).toBe("Lab return draft");
  expect(api.generate).not.toHaveBeenCalled(); expect(api.bindingSet).not.toHaveBeenCalled();
  expect(toLegacyLocation({kind:"project-settings",projectId:"project",section:"generators"}).workspace).toBe("projects");
  expect(toLegacyLocation({kind:"project-settings",projectId:"project",section:"advanced-workflows"}).workspace).toBe("workflows");
  expect(fromLegacyLocation({projectId:"project",workspace:"workflows"})).toEqual({kind:"project-settings",projectId:"project",section:"advanced-workflows"});
});


describe("M2-2 precise library reuse", () => {
  it("sets exact recent provenance, sends it to readiness/generate, clears it on any manual edit", async () => {
    render(<Host />); await loaded();
    fireEvent.click(screen.getByRole("button", {name:"选择提示词"}));
    expect(screen.getByText("近期提示词")).toBeTruthy();
    expect(screen.getByText(/最多20个近期提示词/)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", {name:"我的提示词 · 版本 2"}));
    expect(useStudioStore.getState().creationPromptProvenance).toEqual({promptId:"prm-choice",promptVersionId:"prv-choice"});
    fireEvent.click(screen.getByRole("button", {name:"生成"}));
    await waitFor(()=>expect(api.generate).toHaveBeenCalledWith(expect.objectContaining({promptId:"prm-choice",promptVersionId:"prv-choice"})));
    expect(api.readinessGet).toHaveBeenLastCalledWith(expect.objectContaining({promptId:"prm-choice",promptVersionId:"prv-choice"}));
    fireEvent.change(screen.getByLabelText("提示词"),{target:{value:"chosen prompt!"}});
    expect(useStudioStore.getState().creationPromptProvenance).toBeUndefined();
    fireEvent.change(screen.getByLabelText("提示词"),{target:{value:"chosen prompt"}});
    fireEvent.click(screen.getByRole("button", {name:"生成"}));
    await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
    expect(api.generate.mock.calls[1][0].promptVersionId).toBeUndefined();
  });
  it("captures exact video/shot draft to Library and preserves provenance on return and stage switch", async () => {
    const route={kind:"create",projectId:"project",shotId:"shot2",stage:"video"} as const;
    const navigate=vi.fn(); const view=render(<CreatePage route={route} navigate={navigate}/>); await loaded();
    fireEvent.change(screen.getByLabelText("选择生成器"),{target:{value:"fl-opaque"}});
    fireEvent.change(screen.getByLabelText("宽度"),{target:{value:"1280"}});
    fireEvent.click(screen.getByRole("button",{name:"选择提示词"}));
    fireEvent.click(screen.getByRole("button",{name:"我的提示词 · 版本 2"}));
    fireEvent.click(screen.getByRole("button",{name:"在资源库查找更多图片"}));
    expect(navigate).toHaveBeenLastCalledWith({kind:"library",projectId:"project",filter:"images"});
    expect(useStudioStore.getState().creationLabReturn?.route).toEqual(route);
    const draft=useStudioStore.getState().values; view.unmount();
    render(<CreatePage route={route} navigate={navigate}/>); await loaded();
    expect(useStudioStore.getState().values).toEqual(draft);
    expect(useStudioStore.getState().creationPromptProvenance?.promptVersionId).toBe("prv-choice");
    expect(useStudioStore.getState().draftDirty).toBe(true);
    expect((screen.getByLabelText("选择生成器") as HTMLSelectElement).value).toBe("fl-opaque");
    expect(screen.getByText(/近期100项素材/)).toBeTruthy();
    expect(screen.queryByText(/视频\/音频输入/)).toBeNull();
    expect(api.generate).not.toHaveBeenCalled();
  });
  it("does not inject a late non-recent asset into a different project", async () => {
    let finish!: (detail: unknown)=>void;
    api.libraryGet.mockImplementationOnce(()=>new Promise(resolve=>{finish=resolve;}));
    useStudioStore.getState().setPendingLibraryIntent({kind:"asset",projectId:"project",assetId:"old101",mediaKind:"image"});
    const route={kind:"create",projectId:"project",shotId:"shot1",stage:"video"} as const;
    const view=render(<CreatePage route={route} navigate={navigations}/>); await loaded();
    await waitFor(()=>expect(api.libraryGet).toHaveBeenCalled());
    api.get.mockImplementation((_p,shot,stage)=>Promise.resolve({...context(stage,shot),projectId:"other"}));
    view.rerender(<CreatePage route={{...route,projectId:"other"}} navigate={navigations}/>); await loaded();
    fireEvent.change(screen.getByLabelText("提示词"),{target:{value:"B draft"}});
    await act(async()=>finish({kind:"asset",asset:{id:"old101",name:"old101",projectId:"project",assetType:"image"}}));
    expect(useStudioStore.getState().values.prompt).toEqual({type:"string",value:"B draft"});
    expect(useStudioStore.getState().pendingLibraryIntent).toBeUndefined();
    expect(screen.queryByRole("option",{name:"old101"})).toBeNull();
    expect(api.generate).not.toHaveBeenCalled();
  });
  it("keeps incompatible prompt intent visible without changing generator", async () => {
    api.generatorsList.mockResolvedValue([option("no-prompt",params,"image")]);
    useStudioStore.getState().setPendingLibraryIntent({kind:"prompt",projectId:"project",promptId:"p",promptVersionId:"v",text:"old21",modelVersionId:null});
    render(<Host/>); await loaded();
    expect(screen.getByText(/当前生成器没有可应用提示词的输入/)).toBeTruthy();
    expect(useStudioStore.getState().pendingLibraryIntent?.kind).toBe("prompt");
    expect(useStudioStore.getState().creationPromptProvenance).toBeUndefined();
    expect(api.generate).not.toHaveBeenCalled();
  });
});

it("M2-2 typed prompt provenance survives stage and Settings/Workflow return snapshots", async()=>{
 render(<Host/>); await loaded();
 fireEvent.click(screen.getByRole("button",{name:"选择提示词"}));fireEvent.click(screen.getByRole("button",{name:"我的提示词 · 版本 2"}));
 fireEvent.click(screen.getByRole("button",{name:"视频"})); await loaded();
 expect(useStudioStore.getState().creationPromptProvenance).toBeUndefined();
 fireEvent.click(screen.getByRole("button",{name:"图片"}));await loaded();
 expect(useStudioStore.getState().creationPromptProvenance).toEqual({promptId:"prm-choice",promptVersionId:"prv-choice"});
 fireEvent.click(screen.getByRole("button",{name:"工作流 / Benchmark"}));
 expect(useStudioStore.getState().creationPromptProvenance?.promptVersionId).toBe("prv-choice");
 expect(useStudioStore.getState().creationLabReturn?.scope).toBe("project:shot1:image");
});
