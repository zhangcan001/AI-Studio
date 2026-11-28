// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { GeneratorSettingsPage } from "./GeneratorSettingsPage";
import { GENERATOR_BINDINGS_CHANGED, LAST_GENERATOR_SESSION_KEY, type GeneratorBindingSummary, type GeneratorOption } from "../../product/types";
const api = vi.hoisted(() => ({ overview: vi.fn(), list: vi.fn(), set: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { project: { getOverview: api.overview, generatorBindingSet: api.set }, creation: { generatorsList: api.list } } }));
const native = vi.hoisted(() => ({ listen: vi.fn(), query: vi.fn(), toggle: vi.fn(), importPath: vi.fn() }));
vi.mock("@tauri-apps/api/webview", () => ({getCurrentWebview: () => ({onDragDropEvent: native.listen})}));
vi.mock("../../services/workflowClient", () => ({
 queryWorkflowWorkspace:native.query, setWorkflowEnabled:native.toggle, importWorkflowFromPath:native.importPath,
 commitWorkflowImport:vi.fn(),discardOnboarding:vi.fn(),pickApiWorkflow:vi.fn(),reanalyzeWorkflowImport:vi.fn(),removeWorkflow:vi.fn(),restoreWorkflow:vi.fn(),setOnboardingInputMapping:vi.fn(),setOnboardingMetadata:vi.fn(),setOnboardingOutputMapping:vi.fn(),
}));
const options: GeneratorOption[] = ["one", "two"].map(selectionRef => ({ selectionRef, name: "wfl_krea2_fixture.json", version: "2", mode: "T2V", mediaKind: "video", availability: true, availabilityReason: null, recommended: true, fields: [{ type: "textarea", key: "prompt", label: "Prompt", required: true, default: "" }] }));
let bindings: GeneratorBindingSummary[];
beforeEach(() => { vi.clearAllMocks(); bindings = [{ stage: "VIDEO", mode: "DEFAULT", selectionRef: "one", bindingInstanceId: "instance-A", revision: 3 }]; api.overview.mockImplementation(async () => ({ generatorBindings: bindings })); api.list.mockImplementation(async (_project, stage) => stage === "video" ? options : []); api.set.mockResolvedValue([]); });
afterEach(() => { cleanup(); Reflect.deleteProperty(window, "__TAURI_INTERNALS__"); });
it("phase6_target1 friendly normal projection hides raw identities and requirements use semantic labels", async () => {
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  await screen.findByText("输入要求：提示词");
  expect(document.body.textContent).not.toMatch(/wfl_|instance-A|recipe_|workflowVersionId|nodeId|sha256/);
  expect(api.set).not.toHaveBeenCalled();
});
it("phase6_target2 default binding save uses exact opaque locator and both OCC tokens; environment has canonical return", async () => {
  api.list.mockImplementation(async (_project, stage) => stage === "video" ? [...options, { ...options[0], selectionRef: "video", mode: "fl2va_text_to_video" }] : []);
  const navigate = vi.fn(); render(<GeneratorSettingsPage projectId="p" navigate={navigate} />);
  const select = await screen.findByLabelText("默认视频生成器");
  fireEvent.change(select, { target: { value: "two" } });
  fireEvent.click(screen.getAllByRole("button", { name: "保存设置" })[0]);
  await waitFor(() => expect(api.set).toHaveBeenCalledWith("p", { stage: "VIDEO", mode: "DEFAULT", selectionRef: "two", expectedRevision: 3, expectedBindingInstanceId: "instance-A" }));
  await waitFor(() => expect(screen.getByLabelText("文生视频生成器")).toBeTruthy());
  const mode = screen.getByLabelText("文生视频生成器");
  fireEvent.change(mode, { target: { value: "video" } });
  await waitFor(() => expect(mode.closest("article")?.querySelector("button")?.disabled).toBe(false));
  fireEvent.click(mode.closest("article")!.querySelector("button")!);
  await waitFor(() => expect(api.set).toHaveBeenLastCalledWith("p", { stage: "VIDEO", mode: "FL2VA_TEXT_TO_VIDEO", selectionRef: "video", expectedRevision: null, expectedBindingInstanceId: null }));
  expect(screen.queryByRole("button", { name: /高级工作流/ })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "检查本地运行环境" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "system-settings", section: "general", returnTo: { kind: "project-settings", projectId: "p", section: "generators" } });
});
it("phase6_target3 stale and ABA conflicts refresh read projection without retry or overwrite", async () => {
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  const select = await screen.findByLabelText("默认视频生成器");
  for (const [instance, revision] of [["instance-A", 4], ["instance-B", 1]] as const) {
    api.set.mockRejectedValueOnce({ code: "GENERATOR_BINDING_CONFLICT", message: "technical recipe_unsafe" });
    fireEvent.change(select, { target: { value: "two" } });
    bindings = [{ stage: "VIDEO", mode: "DEFAULT", selectionRef: "one", bindingInstanceId: instance, revision }];
    fireEvent.click(screen.getAllByRole("button", { name: "保存设置" })[0]);
    await screen.findByRole("alert"); await waitFor(() => expect((select as HTMLSelectElement).value).toBe("one"));
    expect(document.body.textContent).not.toContain("recipe_unsafe");
    fireEvent.click(screen.getByRole("button", { name: "刷新设置" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  }
  expect(api.set).toHaveBeenCalledTimes(2);
  expect(api.set.mock.calls[1][1]).toMatchObject({ expectedRevision: 4, expectedBindingInstanceId: "instance-A" });
});
it("retired image generators are not offered; IMAGE binding data is untouched", async () => {
  const image = { ...options[0], selectionRef: "img", mode: "T2I", mediaKind: "image" as const };
  api.list.mockImplementation(async (_project, stage) => stage === "video" ? options : [image]);
  bindings = [...bindings, { stage: "IMAGE", mode: "DEFAULT", selectionRef: "img", bindingInstanceId: "i", revision: 1 }];
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  await screen.findByLabelText("默认视频生成器");
  expect(screen.queryByLabelText("默认图片生成器")).toBeNull();
  expect(api.list).not.toHaveBeenCalledWith("p", "image");
  expect(api.set).not.toHaveBeenCalled();
});
it("empty video catalog directs users to builtin updates/environment, never import", async () => {
  api.list.mockImplementation(async () => []);
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  const empty = (await screen.findByTestId("no-video-generators")).textContent;
  expect(empty).toContain("内置生成器");
  expect(empty).not.toMatch(/高级工作流|导入并验证/);
});
it("imported user video generators are selectable; stray image options never reach the video slot", async () => {
  const user = { ...options[0], selectionRef: "user-i2v", name: "My I2V", mode: "image_to_video" };
  const stray = { ...options[0], selectionRef: "stray-img", mode: "T2I", mediaKind: "image" as const };
  api.list.mockImplementation(async (_project, stage) => stage === "video" ? [user, stray] : []);
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  const select = await screen.findByLabelText("默认视频生成器") as HTMLSelectElement;
  const values = [...select.options].map(o => o.value);
  expect(values).toContain("user-i2v"); expect(values).not.toContain("stray-img");
});

it("marks dirty generator slots and batch-saves all modified bindings with exact OCC tokens", async () => {
  api.list.mockImplementation(async (_project, stage) => stage === "video" ? [...options, { ...options[0], selectionRef: "video-t2v", mode: "fl2va_text_to_video" }] : []);
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  const def = await screen.findByLabelText("默认视频生成器");
  const t2v = await screen.findByLabelText("文生视频生成器");
  fireEvent.change(def, { target: { value: "two" } });
  fireEvent.change(t2v, { target: { value: "video-t2v" } });
  expect(screen.getAllByText("未保存修改")).toHaveLength(2);
  fireEvent.click(screen.getByRole("button", { name: "保存全部修改（2）" }));
  await waitFor(() => expect(api.set).toHaveBeenCalledTimes(2));
  expect(api.set).toHaveBeenNthCalledWith(1, "p", { stage: "VIDEO", mode: "DEFAULT", selectionRef: "two", expectedRevision: 3, expectedBindingInstanceId: "instance-A" });
  expect(api.set).toHaveBeenNthCalledWith(2, "p", { stage: "VIDEO", mode: "FL2VA_TEXT_TO_VIDEO", selectionRef: "video-t2v", expectedRevision: null, expectedBindingInstanceId: null });
});

it("retains the failed and untouched generator choices and retries only those slots", async () => {
  api.list.mockResolvedValue([...options,
    { ...options[0], selectionRef: "video-t2v", mode: "fl2va_text_to_video" },
    { ...options[0], selectionRef: "video-i2v", mode: "fl2va_image_to_video" },
  ]);
  api.set.mockImplementationOnce(async () => {
    bindings = [{ ...bindings[0], selectionRef: "two", revision: 4 }];
  }).mockRejectedValueOnce({ code: "GENERATOR_BINDING_CONFLICT", message: "revision" });
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  const defaultSlot = await screen.findByLabelText("默认视频生成器") as HTMLSelectElement;
  const textSlot = await screen.findByLabelText("文生视频生成器") as HTMLSelectElement;
  const imageSlot = await screen.findByLabelText("图生视频生成器") as HTMLSelectElement;
  fireEvent.change(defaultSlot, { target: { value: "two" } });
  fireEvent.change(textSlot, { target: { value: "video-t2v" } });
  fireEvent.change(imageSlot, { target: { value: "video-i2v" } });
  fireEvent.click(screen.getByRole("button", { name: "保存全部修改（3）" }));
  expect((await screen.findByRole("alert")).textContent).toContain("已保存 1 项");
  const retry = await screen.findByRole("button", { name: "保存全部修改（2）" });
  expect(textSlot.value).toBe("video-t2v");
  expect(imageSlot.value).toBe("video-i2v");
  await waitFor(() => expect((retry as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(retry);
  await waitFor(() => expect(api.set).toHaveBeenCalledTimes(4));
  expect(api.set.mock.calls.slice(2).map(([, request]) => request.selectionRef)).toEqual(["video-t2v", "video-i2v"]);
});

it("stops saveAll remaining writes when switching projects mid-batch", async () => {
  api.list.mockResolvedValue([...options,
    { ...options[0], selectionRef: "video-t2v", mode: "fl2va_text_to_video" },
  ]);
  let resolveFirst!: () => void;
  api.set.mockImplementationOnce(() => new Promise<void>(resolve => { resolveFirst = resolve; }));
  const view = render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  fireEvent.change(await screen.findByLabelText("默认视频生成器"), { target: { value: "two" } });
  fireEvent.change(screen.getByLabelText("文生视频生成器"), { target: { value: "video-t2v" } });
  fireEvent.click(screen.getByRole("button", { name: "保存全部修改（2）" }));
  await waitFor(() => expect(api.set).toHaveBeenCalledTimes(1));
  view.rerender(<GeneratorSettingsPage projectId="p2" navigate={vi.fn()} />);
  await waitFor(() => expect(api.overview).toHaveBeenCalledWith("p2"));
  resolveFirst();
  await new Promise(resolve => setTimeout(resolve, 20));
  expect(api.set).toHaveBeenCalledTimes(1);
});

it("preserves remaining unsaved choices across consecutive single-slot conflicts and refreshes", async () => {
  api.list.mockResolvedValue([
    ...options,
    { ...options[0], selectionRef: "video-t2v", mode: "fl2va_text_to_video" },
    { ...options[0], selectionRef: "video-i2v", mode: "fl2va_image_to_video" },
  ]);
  api.set
    .mockImplementationOnce(async () => {
      bindings = [{ ...bindings[0], selectionRef: "two", revision: 4 }];
    })
    .mockRejectedValueOnce({ code: "GENERATOR_BINDING_CONFLICT", message: "conflict 1" })
    .mockRejectedValueOnce({ code: "GENERATOR_BINDING_CONFLICT", message: "conflict 2" })
    .mockRejectedValueOnce({ code: "GENERATOR_BINDING_CONFLICT", message: "conflict 3" })
    .mockResolvedValueOnce([]);
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  const defaultSlot = await screen.findByLabelText("默认视频生成器") as HTMLSelectElement;
  const textSlot = await screen.findByLabelText("文生视频生成器") as HTMLSelectElement;
  const imageSlot = await screen.findByLabelText("图生视频生成器") as HTMLSelectElement;
  fireEvent.change(defaultSlot, { target: { value: "two" } });
  fireEvent.change(textSlot, { target: { value: "video-t2v" } });
  fireEvent.change(imageSlot, { target: { value: "video-i2v" } });
  fireEvent.click(screen.getByRole("button", { name: "保存全部修改（3）" }));
  expect((await screen.findByRole("alert")).textContent).toContain("已保存 1 项");

  const textSaveBtn = textSlot.closest("article")!.querySelector("button")!;
  expect(textSaveBtn.disabled).toBe(false);
  fireEvent.click(textSaveBtn);
  await screen.findByRole("alert");
  expect(textSaveBtn.disabled).toBe(true);
  expect(textSlot.value).toBe("video-t2v");
  expect(imageSlot.value).toBe("video-i2v");

  fireEvent.click(screen.getByRole("button", { name: "刷新设置" }));
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  expect(textSlot.value).toBe("video-t2v");
  expect(imageSlot.value).toBe("video-i2v");

  fireEvent.click(textSaveBtn);
  await screen.findByRole("alert");
  expect(textSlot.value).toBe("video-t2v");
  expect(imageSlot.value).toBe("video-i2v");

  fireEvent.click(screen.getByRole("button", { name: "刷新设置" }));
  await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  fireEvent.click(textSaveBtn);
  await waitFor(() => expect(api.set).toHaveBeenCalledTimes(5));
  expect(imageSlot.value).toBe("video-i2v");
});

it("locks controls while manual refresh is in flight to avoid racing with saves", async () => {
  api.overview.mockResolvedValue({ generators: [{ selectionRef: "video-t2v", name: "Wan 2.2", mode: "T2V", mediaKind: "video", version: "1", availability: true, fields: [] }] });
  api.list.mockResolvedValue([]);
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  const refreshBtn = await screen.findByRole("button", { name: "刷新设置" });

  let resolveOverview!: (v: unknown) => void;
  api.overview.mockReturnValueOnce(new Promise(resolve => { resolveOverview = resolve; }));
  fireEvent.click(refreshBtn);
  expect(refreshBtn.hasAttribute("disabled")).toBe(true);
  fireEvent.click(refreshBtn);
  expect(api.overview).toHaveBeenCalledTimes(2);

  resolveOverview({ generators: [{ selectionRef: "video-t2v", name: "Wan 2.2", mode: "T2V", mediaKind: "video", version: "1", availability: true, fields: [] }] });
  await waitFor(() => expect(refreshBtn.hasAttribute("disabled")).toBe(false));
});

it("disables manual refresh during initial load and ignores a stale initial response", async () => {
  type OverviewResult = { generatorBindings: GeneratorBindingSummary[] };
  let resolveStale!: (v: OverviewResult) => void;
  api.overview.mockImplementationOnce(() => new Promise<OverviewResult>(resolve => { resolveStale = resolve; }));

  const ui = render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  const refreshBtn = await screen.findByRole("button", { name: "刷新设置" }) as HTMLButtonElement;
  expect(refreshBtn.disabled).toBe(true);
  fireEvent.click(refreshBtn);
  expect(api.overview).toHaveBeenCalledTimes(1);

  api.overview.mockResolvedValue({
    generatorBindings: [{ stage: "VIDEO", mode: "DEFAULT", selectionRef: "one", bindingInstanceId: "instance-A", revision: 4 }],
  });
  ui.rerender(<GeneratorSettingsPage projectId="p2" navigate={vi.fn()} />);
  await screen.findByLabelText("默认视频生成器");
  ui.rerender(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  const select = await screen.findByLabelText("默认视频生成器");

  resolveStale({
    generatorBindings: [{ stage: "VIDEO", mode: "DEFAULT", selectionRef: "one", bindingInstanceId: "instance-A", revision: 3 }],
  });
  await waitFor(() => expect((screen.getByRole("button", { name: "刷新设置" }) as HTMLButtonElement).disabled).toBe(false));

  fireEvent.change(select, { target: { value: "two" } });
  fireEvent.click(screen.getAllByRole("button", { name: "保存设置" })[0]);
  await waitFor(() => expect(api.set).toHaveBeenCalledWith("p", expect.objectContaining({ expectedRevision: 4 })));
});

it("treats committed save as saved even if post-save refresh fails, clears stale session key and notifies listeners", async () => {
  api.list.mockResolvedValue([
    ...options,
    { ...options[0], selectionRef: "video-t2v", mode: "fl2va_text_to_video" },
  ]);
  sessionStorage.setItem(`${LAST_GENERATOR_SESSION_KEY}:p:video`, "one");
  const listener = vi.fn();
  window.addEventListener(GENERATOR_BINDINGS_CHANGED, listener);
  try {
    api.set
      .mockImplementationOnce(async () => {
        bindings = [{ ...bindings[0], selectionRef: "two", revision: 4 }];
      })
      .mockRejectedValueOnce({ code: "GENERATOR_BINDING_CONFLICT", message: "conflict" })
      .mockResolvedValueOnce([]);
    render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
    const defaultSlot = await screen.findByLabelText("默认视频生成器") as HTMLSelectElement;
    const textSlot = await screen.findByLabelText("文生视频生成器") as HTMLSelectElement;

    // Partial saveAll retains VIDEO:FL2VA_TEXT_TO_VIDEO in retainedConflictKeysRef
    fireEvent.change(defaultSlot, { target: { value: "two" } });
    fireEvent.change(textSlot, { target: { value: "video-t2v" } });
    fireEvent.click(screen.getByRole("button", { name: "保存全部修改（2）" }));
    expect((await screen.findByRole("alert")).textContent).toContain("已保存 1 项");

    // Retrying single save commits, then follow-up refresh fails
    api.overview.mockRejectedValueOnce(new Error("temporary refresh failure"));
    const textSaveBtn = textSlot.closest("article")!.querySelector("button")!;
    fireEvent.click(textSaveBtn);

    expect(await screen.findByText("生成器设置已保存，但读取最新设置失败；请点击刷新设置。")).toBeTruthy();
    expect(screen.queryByText("未保存修改")).toBeNull();
    expect(sessionStorage.getItem(`${LAST_GENERATOR_SESSION_KEY}:p:video`)).toBeNull();
    expect(listener).toHaveBeenCalled();
  } finally {
    window.removeEventListener(GENERATOR_BINDINGS_CHANGED, listener);
    sessionStorage.clear();
  }
});

it("reports unsaved dirty state via onDirtyChange and clears it after saving", async () => {
  const onDirtyChange = vi.fn();
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} onDirtyChange={onDirtyChange} />);
  const select = await screen.findByLabelText("默认视频生成器");
  expect(onDirtyChange).toHaveBeenLastCalledWith(false);

  fireEvent.change(select, { target: { value: "two" } });
  await waitFor(() => expect(onDirtyChange).toHaveBeenLastCalledWith(true));

  fireEvent.click(screen.getByRole("button", { name: "保存设置" }));
  await waitFor(() => expect(onDirtyChange).toHaveBeenLastCalledWith(false));
});

it("uses the exact current version and recipe for lifecycle status and action", async () => {
 Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{},configurable:true});
 native.listen.mockResolvedValue(()=>{});
 native.query.mockResolvedValue({items:[{registry:{workflowId:"w",name:"Multi-version custom",sourceKind:"USER",currentVersionId:"v2",currentRecipe:{recipeId:"r2"}},runtime:[{workflowVersionId:"v1",recipeId:"r1",enabled:false},{workflowVersionId:"v2",recipeId:"r-old",enabled:false},{workflowVersionId:"v2",recipeId:"r2",enabled:true}]}]});
 native.toggle.mockResolvedValue(undefined);
 render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()}/>);
 await screen.findByText("Multi-version custom");
 fireEvent.click(screen.getByRole("button",{name:/^停用$/}));
 await waitFor(()=>expect(native.toggle).toHaveBeenCalledWith("v2",false));
});
it("serializes native drops while a workflow import is still unresolved", async () => {
 Object.defineProperty(window,"__TAURI_INTERNALS__",{value:{},configurable:true});
 native.listen.mockResolvedValue(()=>{}); native.query.mockResolvedValue({items:[]});
 native.importPath.mockImplementation(()=>new Promise(()=>{}));
 render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()}/>);
 await waitFor(()=>expect(native.listen).toHaveBeenCalledTimes(1));
 const callback=native.listen.mock.calls[0][0];
 await act(async()=>{callback({payload:{type:"drop",paths:["C:/one.json"]}});});
 await act(async()=>{callback({payload:{type:"drop",paths:["C:/two.json"]}});});
 expect(native.importPath).toHaveBeenCalledTimes(1);
});
