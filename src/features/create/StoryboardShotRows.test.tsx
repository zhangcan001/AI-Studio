// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import { StoryboardShotRows } from "./StoryboardShotRows";
import type { StoryboardShot } from "./storyboardModel";
import { normalizeProductError } from "../../product/errors";
import { invalidateVideoInputs } from "../../product/videoInputInvalidation";

const api = vi.hoisted(() => ({ updateShot: vi.fn(), deleteShot: vi.fn(), readinessGet: vi.fn(), generate: vi.fn(), selectResult: vi.fn(), list: vi.fn(), listForShot: vi.fn(), bindShot: vi.fn(), generatorsList: vi.fn(), inputsGet: vi.fn(), inputsSave: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: {
  creation: { updateShot: api.updateShot, deleteShot: api.deleteShot, readinessGet: api.readinessGet, generate: api.generate, selectResult: api.selectResult, generatorsList: api.generatorsList, inputsGet: api.inputsGet, inputsSave: api.inputsSave, mediaUrl: (p: string, id: string) => `media://${p}/${id}` },
  storyEntities: { list: api.list, listForShot: api.listForShot, bindShot: api.bindShot },
  library: { imageGet: vi.fn(async () => [1]), thumbnailGet: vi.fn(async () => null) },
} }));
const hero = { id: "e1", projectId: "p", kind: "character", name: "阿离", description: "", coverAssetId: null, imageAssetIds: [], createdAt: "", updatedAt: "" } as const;
const lane = { ...hero, id: "e2", kind: "scene", name: "雨巷" } as const;
const shot = (id: string, ordinal: number, selectionRef: string | null = "sel"): StoryboardShot => ({ id, name: `镜头${ordinal + 1}`, ordinal, status: "TODO", candidates: [], run: null,
  shot: { summary: { id, name: `镜头${ordinal + 1}`, ordinal }, prompt: "阿离 走进 雨巷", selectionRef, values: { duration_seconds: { type: "integer", value: 5 } }, referenceAssetIds: [], selectedResultId: null, recentRun: null } });
const generator = { selectionRef: "sel", name: "H3", version: "1", mode: "t2v", mediaKind: "video", availability: true, availabilityReason: null, recommended: true,
  fields: [{ key: "duration_seconds", type: "integer", label: "时长", required: false, min: 1, max: 10 }],
  resolutionPresets: [{ id: "mp-0_98-16x9", label: "0.98MP · 1344×768", width: 1344, height: 768 }, { id: "mp-0_98-9x16", label: "0.98MP · 768×1344", width: 768, height: 1344 }, { id: "mp-2_09-9x16", label: "2.09MP · 1088×1920", width: 1088, height: 1920 }] };
const props = () => ({ projectId: "p", busy: false, onOpen: vi.fn(), onCreate: vi.fn(), onChanged: vi.fn(), onQueue: vi.fn() });
beforeEach(() => {
  api.list.mockResolvedValue([hero, lane]); api.listForShot.mockResolvedValue([hero]);
  api.bindShot.mockImplementation(async (_p: string, _s: string, ids: string[]) => [hero, lane].filter(e => ids.includes(e.id)));
  api.readinessGet.mockResolvedValue({ ready: true, issues: [], fieldErrors: [], actions: [] });
  api.generate.mockResolvedValue({ accepted: true, runRef: { source: "queue-batch", id: "r" }, startOutcome: "QUEUED", startIssue: null });
  api.generatorsList.mockResolvedValue([generator]);
  api.inputsGet.mockImplementation(async s => ({ selectionRef: s.selectionRef, token: { instanceId: 'refs', revision: 4 }, inputs: [], assets: [] }));
  api.inputsSave.mockImplementation(async r => ({ selectionRef: r.selection.selectionRef, token: { instanceId: 'refs', revision: 5 }, inputs: r.inputs, assets: [] }));
  api.updateShot.mockResolvedValue({}); api.deleteShot.mockResolvedValue(undefined);
});
afterEach(() => { cleanup(); vi.clearAllMocks(); vi.restoreAllMocks(); });

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(yes => { resolve = yes; });
  return { promise, resolve };
}

it("keeps narrow panel rows usable and enables wide columns by container width, not window width", () => {
  // CSS contract only: real Native geometry is verified separately, not simulated by jsdom.
  const css = readFileSync("src/features/create/CreatePage.css", "utf8").replace(/\s+/g, "");
  expect(/\.storyboard-shot-rows\{[^}]*container:shot-rows\/inline-size/.test(css), "panel-scoped size container").toBe(true);
  expect(/\.shot-row-card\{[^}]*grid-template-columns:32pxminmax\(0,1fr\)110px/.test(css), "usable narrow fallback").toBe(true);
  for (const part of ["prompt", "fields"]) {
    expect(new RegExp(`\\.shot-row-${part}\\{[^}]*grid-column:2/-1`).test(css), `${part} spans the narrow row`).toBe(true);
  }
  expect(css.includes("@containershot-rows(min-width:900px){.shot-row-card{"), "wide columns require enough panel width").toBe(true);
  expect(css.includes(".shot-row-prompt,.shot-row-fields{grid-column:auto}"), "restore wide placement").toBe(true);
  expect(/@media[^{}]*\{\.shot-row-card\{/.test(css), "no viewport-only row breakpoint").toBe(false);
});

it("does not replace saved bindings until the complete initial read succeeds", async () => {
  const initial = deferred<(typeof hero | typeof lane)[]>();
  api.listForShot.mockReturnValue(initial.promise);
  render(<StoryboardShotRows {...props()} items={[shot("a", 0)]} />);
  const add = await screen.findByTitle("绑定 场景 · 雨巷");
  expect(add.hasAttribute("disabled")).toBe(true);
  expect(screen.getByLabelText("镜头1 提示词").hasAttribute("disabled")).toBe(true);
  fireEvent.click(add);
  expect(api.bindShot).not.toHaveBeenCalled();
  await act(async () => { initial.resolve([hero]); });
  await waitFor(() => expect(add.hasAttribute("disabled")).toBe(false));
  fireEvent.click(add);
  await waitFor(() => expect(api.bindShot).toHaveBeenCalledWith("p", "a", ["e1", "e2"]));
});

it("fails closed when saved bindings cannot be read", async () => {
  api.listForShot.mockRejectedValue(new Error("binding read failed"));
  render(<StoryboardShotRows {...props()} items={[shot("a", 0)]} />);
  const add = await screen.findByTitle("绑定 场景 · 雨巷");
  await screen.findByRole("alert");
  expect(add.hasAttribute("disabled")).toBe(true);
  fireEvent.click(add);
  expect(api.bindShot).not.toHaveBeenCalled();
});

it("ignores a superseded binding read instead of removing newer binding state", async () => {
  const old = deferred<(typeof hero | typeof lane)[]>();
  api.listForShot.mockReturnValueOnce(old.promise).mockResolvedValue([hero, lane]);
  render(<StoryboardShotRows {...props()} items={[shot("a", 0)]} />);
  await waitFor(() => expect(api.listForShot.mock.calls.length).toBeGreaterThanOrEqual(2));
  await screen.findByTitle("解除绑定 场景 · 雨巷");
  await act(async () => { old.resolve([]); });
  expect(screen.getByTitle("解除绑定 场景 · 雨巷")).toBeTruthy();
  fireEvent.click(screen.getByTitle("解除绑定 角色 · 阿离"));
  await waitFor(() => expect(api.bindShot).toHaveBeenCalledWith("p", "a", ["e2"]));
});

it.each(["unmount", "project", "draft"])("does not submit late READY after a %s change", async change => {
  const pending = deferred<{ ready: boolean; issues: []; fieldErrors: []; actions: [] }>();
  api.readinessGet.mockReturnValueOnce(pending.promise);
  const p = props();
  const view = render(<StoryboardShotRows {...p} items={[shot("a", 0)]} />);
  fireEvent.click(await screen.findByRole("button", { name: "生成视频" }));
  await waitFor(() => expect(api.readinessGet).toHaveBeenCalledTimes(1));
  if (change === "unmount") view.unmount();
  else if (change === "project") view.rerender(<StoryboardShotRows {...p} projectId="other" items={[shot("a", 0)]} />);
  else { const next = shot("a", 0); next.shot!.values = { duration_seconds: { type: "integer", value: 8 } }; view.rerender(<StoryboardShotRows {...p} items={[next]} />); }
  await act(async () => { pending.resolve({ ready: true, issues: [], fieldErrors: [], actions: [] }); });
  expect(api.generate).not.toHaveBeenCalled();
});

it("preserves dirty duration across same-value and changed-value refreshes, then resumes syncing after save", async () => {
  const p = props(); const view = render(<StoryboardShotRows {...p} items={[shot("a", 0)]} />);
  const input = await screen.findByLabelText("镜头1 时长（秒）") as HTMLInputElement;
  fireEvent.change(input, { target: { value: "7" } });
  view.rerender(<StoryboardShotRows {...p} items={[shot("a", 0)]} />);
  expect(input.value).toBe("7");
  const refreshed = shot("a", 0); refreshed.shot!.values = { duration_seconds: { type: "integer", value: 8 } };
  view.rerender(<StoryboardShotRows {...p} items={[refreshed]} />);
  expect(input.value).toBe("7");
  expect(api.updateShot).not.toHaveBeenCalled();
  fireEvent.blur(input);
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith("p", expect.objectContaining({ values: { duration_seconds: { type: "integer", value: 7 } } })));
  await screen.findByRole("button", { name: "生成视频" });
  await waitFor(() => expect(screen.getByLabelText("镜头1 时长（秒）").hasAttribute("disabled")).toBe(false));
  const saved = shot("a", 0); saved.shot!.values = { duration_seconds: { type: "integer", value: 9 } };
  view.rerender(<StoryboardShotRows {...p} items={[saved]} />);
  expect(input.value).toBe("9");
});

it("renders real shot rows with structured description, inline @entity chips and search/status filter", async () => {
  const withMentions = shot("a", 0); withMentions.shot!.prompt = "@阿离 走进 @雨巷";
  render(<StoryboardShotRows {...props()} items={[withMentions, { ...shot("b", 1), status: "COMPLETED", name: "收尾" }]} />);
  const row = await screen.findByRole("article", { name: "分镜 1 镜头1" });
  await waitFor(() => expect(within(within(row).getByLabelText("分镜描述")).getAllByText("阿离").length).toBeGreaterThan(0));
  expect(within(within(row).getByLabelText("分镜描述")).getByText("雨巷")).toBeTruthy();
  expect(within(row).getByText("5 秒", { selector: "dd" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "已完成" }));
  expect(screen.queryByRole("article", { name: "分镜 1 镜头1" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "全部" }));
  fireEvent.change(screen.getByLabelText("搜索分镜"), { target: { value: "收尾" } });
  expect(screen.getAllByRole("article")).toHaveLength(1);
  expect(screen.queryByText(/批量数量|推理|插件/)).toBeNull();
});

it("binds/unbinds entities, renames via updateShot, deletes with confirm and generates through readiness + Production Queue", async () => {
  const p = props();
  render(<StoryboardShotRows {...p} items={[shot("a", 0)]} />);
  const row = await screen.findByRole("article", { name: "分镜 1 镜头1" });
  const list = within(row).getByLabelText("项目角色与场景");
  await within(list).findByTitle("解除绑定 角色 · 阿离");
  fireEvent.click(within(list).getByTitle("绑定 场景 · 雨巷"));
  await waitFor(() => expect(api.bindShot).toHaveBeenCalledWith("p", "a", ["e1", "e2"]));
  fireEvent.click(await within(list).findByTitle("解除绑定 角色 · 阿离"));
  await waitFor(() => expect(api.bindShot).toHaveBeenLastCalledWith("p", "a", ["e2"]));
  const nameInput = within(row).getByLabelText("镜头1 名称");
  fireEvent.change(nameInput, { target: { value: "开场" } }); fireEvent.blur(nameInput);
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith("p", { shotId: "a", name: "开场" }));
  fireEvent.change(within(row).getByLabelText("镜头1 提示词"), { target: { value: "新的提示词" } });
  fireEvent.click(within(row).getByRole("button", { name: "生成视频" }));
  await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(1));
  expect(api.readinessGet.mock.calls[0][0]).toMatchObject({ projectId: "p", shotId: "a", stage: "video", selectionRef: "sel", submissionIdempotencyKey: "readiness-only" });
  expect(api.generate.mock.calls[0][0].values.prompt).toEqual({ type: "string", value: "新的提示词" });
  expect(api.generate.mock.calls[0][0].submissionIdempotencyKey).not.toBe("readiness-only");
  await within(row).findByText("已写入生产队列");
  const confirm = vi.spyOn(window, "confirm").mockReturnValueOnce(false).mockReturnValueOnce(true);
  fireEvent.click(within(row).getByRole("button", { name: "删除分镜" }));
  expect(api.deleteShot).not.toHaveBeenCalled();
  fireEvent.click(within(row).getByRole("button", { name: "删除分镜" }));
  await waitFor(() => expect(api.deleteShot).toHaveBeenCalledWith("p", "a"));
  expect(confirm).toHaveBeenCalledTimes(2);
  fireEvent.click(within(row).getByRole("button", { name: "新建分镜" })); expect(p.onCreate).toHaveBeenCalled();
  fireEvent.click(within(row).getByRole("button", { name: "详情" })); expect(p.onOpen).toHaveBeenCalled();
});

it("blocks generation when readiness fails and when no generator is selected", async () => {
  api.readinessGet.mockResolvedValueOnce({ ready: false, issues: [{ code: "X", message: "缺少首帧", details: {} }], fieldErrors: [], actions: [] });
  render(<StoryboardShotRows {...props()} items={[shot("a", 0), shot("b", 1, null)]} />);
  const first = await screen.findByRole("article", { name: "分镜 1 镜头1" });
  fireEvent.click(within(first).getByRole("button", { name: "生成视频" }));
  expect(await within(first).findByRole("alert")).toHaveProperty("textContent", "缺少首帧");
  expect(api.generate).not.toHaveBeenCalled();
  const second = screen.getByRole("article", { name: "分镜 2 镜头2" });
  expect(within(second).getByRole("button", { name: "生成视频" }).hasAttribute("disabled")).toBe(true);
});

it("saves the prompt on blur and auto-fills recognized duration, aspect/resolution and entity bindings with undo", async () => {
  const p = props();
  render(<StoryboardShotRows {...p} items={[shot("a", 0)]} />);
  const row = await screen.findByRole("article", { name: "分镜 1 镜头1" });
  await within(row).findByLabelText("镜头1 画幅");
  await within(within(row).getByLabelText("项目角色与场景")).findByTitle("解除绑定 角色 · 阿离");
  const box = within(row).getByLabelText("镜头1 提示词");
  fireEvent.change(box, { target: { value: "阿离 走进 雨巷 8秒 竖屏 720p" } });
  expect(within(row).getByLabelText("识别结果").textContent).toContain("时长 8 秒");
  expect(within(row).getByLabelText("识别结果").textContent).toContain("画幅 9:16 · 768×1344");
  expect(within(row).getByLabelText("识别结果").textContent).toContain("场景 @雨巷");
  fireEvent.blur(box);
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith("p", { shotId: "a", name: "镜头1", stage: "video", prompt: "阿离 走进 雨巷 8秒 竖屏 720p", selectionRef: "sel",
    values: { duration_seconds: { type: "integer", value: 8 }, width: { type: "integer", value: 768 }, height: { type: "integer", value: 1344 } } }));
  await waitFor(() => expect(api.bindShot).toHaveBeenCalledWith("p", "a", ["e1", "e2"]));
  const status = (await within(row).findByText(/已识别并填入/)).closest("p")!;
  expect(status.textContent).toContain("已识别并填入：时长 8 秒 · 画幅 9:16 · 768×1344 · 场景 @雨巷");
  expect(p.onChanged).toHaveBeenCalled();
  fireEvent.click(within(status).getByRole("button", { name: "撤销" }));
  await waitFor(() => expect(api.updateShot).toHaveBeenLastCalledWith("p", { shotId: "a", name: "镜头1", stage: "video", selectionRef: "sel", values: { duration_seconds: { type: "integer", value: 5 } } }));
  await waitFor(() => expect(api.bindShot).toHaveBeenLastCalledWith("p", "a", ["e1"]));
  await within(row).findByText("已撤销自动填入");
});

it("edits duration, aspect and resolution inline through updateShot; Ctrl+Enter saves the prompt", async () => {
  render(<StoryboardShotRows {...props()} items={[shot("a", 0)]} />);
  const row = await screen.findByRole("article", { name: "分镜 1 镜头1" });
  const duration = await within(row).findByLabelText("镜头1 时长（秒）");
  expect((duration as HTMLInputElement).value).toBe("5");
  fireEvent.change(duration, { target: { value: "7" } }); fireEvent.blur(duration);
  await waitFor(() => expect(api.updateShot).toHaveBeenLastCalledWith("p", { shotId: "a", name: "镜头1", stage: "video", selectionRef: "sel", values: { duration_seconds: { type: "integer", value: 7 } } }));
  fireEvent.change(within(row).getByLabelText("镜头1 画幅"), { target: { value: "16:9" } });
  await waitFor(() => expect(api.updateShot).toHaveBeenLastCalledWith("p", expect.objectContaining({ values: { duration_seconds: { type: "integer", value: 5 }, width: { type: "integer", value: 1344 }, height: { type: "integer", value: 768 } } })));
  fireEvent.change(within(row).getByLabelText("镜头1 分辨率"), { target: { value: "mp-2_09-9x16" } });
  await waitFor(() => expect(api.updateShot).toHaveBeenLastCalledWith("p", expect.objectContaining({ values: expect.objectContaining({ width: { type: "integer", value: 1088 }, height: { type: "integer", value: 1920 } }) })));
  const box = within(row).getByLabelText("镜头1 提示词");
  fireEvent.change(box, { target: { value: "只改文字" } });
  fireEvent.keyDown(box, { key: "Enter", ctrlKey: true });
  await waitFor(() => expect(api.updateShot).toHaveBeenLastCalledWith("p", { shotId: "a", name: "镜头1", stage: "video", prompt: "只改文字" }));
  expect(await within(row).findByText("提示词已保存")).toBeTruthy();
});

it("uses the default generator for a shot without stage config so the first inline edit creates it", async () => {
  render(<StoryboardShotRows {...props()} items={[shot("c", 0, null)]} />);
  const row = await screen.findByRole("article", { name: "分镜 1 镜头1" });
  fireEvent.change(await within(row).findByLabelText("镜头1 画幅"), { target: { value: "9:16" } });
  await waitFor(() => expect(api.updateShot).toHaveBeenLastCalledWith("p", { shotId: "c", name: "镜头1", stage: "video", selectionRef: "sel",
    values: { duration_seconds: { type: "integer", value: 5 }, width: { type: "integer", value: 768 }, height: { type: "integer", value: 1344 } } }));
  expect(within(row).getByRole("button", { name: "生成视频" }).hasAttribute("disabled")).toBe(true);
});

it("shows read-only duration/aspect when no generator exists and never sends values", async () => {
  api.generatorsList.mockResolvedValue([]);
  render(<StoryboardShotRows {...props()} items={[shot("b", 0, null)]} />);
  const row = await screen.findByRole("article", { name: "分镜 1 镜头1" });
  await waitFor(() => expect(api.generatorsList).toHaveBeenCalled());
  expect(within(row).queryByLabelText("镜头1 画幅")).toBeNull();
  const box = within(row).getByLabelText("镜头1 提示词");
  await waitFor(() => expect(box.hasAttribute("disabled")).toBe(false));
  fireEvent.change(box, { target: { value: "竖屏 6秒" } }); fireEvent.blur(box);
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith("p", { shotId: "b", name: "镜头1", stage: "video", prompt: "竖屏 6秒" }));
});

it("explains an incomplete width-only setting instead of silently ignoring it", async () => {
  render(<StoryboardShotRows {...props()} items={[shot("width-only", 0)]} />);
  const box = await screen.findByLabelText("镜头1 提示词");
  await waitFor(() => expect(box.hasAttribute("disabled")).toBe(false));
  fireEvent.change(box, { target: { value: "画幅1920" } });
  expect(screen.getByRole("status", { name: "提示词识别提示" }).textContent).toContain("画幅/分辨率信息不完整");
  fireEvent.blur(box);
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith("p", { shotId: "width-only", name: "镜头1", stage: "video", prompt: "画幅1920" }));
  expect(api.generate).not.toHaveBeenCalled();
});

it.each(["0.5MP", "50万像素", "总像素500000"])("saves %s as approved dimensions, not a width or a new GPU task", async text => {
  api.generatorsList.mockResolvedValue([{ ...generator, resolutionPresets: [...generator.resolutionPresets,
    { id: "mp-0_5-16x9", label: "0.5MP · 960×544", width: 960, height: 544 },
  ] }]);
  render(<StoryboardShotRows {...props()} items={[shot("pixels", 0)]} />);
  const box = await screen.findByLabelText("镜头1 提示词");
  await waitFor(() => expect(box.hasAttribute("disabled")).toBe(false));
  fireEvent.change(box, { target: { value: text } }); fireEvent.blur(box);
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith("p", expect.objectContaining({
    shotId: "pixels", prompt: text, selectionRef: "sel", values: expect.objectContaining({ width: { type: "integer", value: 960 }, height: { type: "integer", value: 544 } }),
  })));
  expect((await screen.findByText(/已识别并填入/)).textContent).toContain("522240 像素");
  expect(api.generate).not.toHaveBeenCalled();
});

it.each([
  ["画幅：9:16", 768, 1344],
  ["画幅９：１６", 768, 1344],
  ["分辨率1920×1080", 1344, 768],
])("writes %s as actual supported Recipe dimensions without generation", async (text, width, height) => {
  render(<StoryboardShotRows {...props()} items={[shot("explicit-size", 0)]} />);
  const box = await screen.findByLabelText("镜头1 提示词");
  await waitFor(() => expect(box.hasAttribute("disabled")).toBe(false));
  fireEvent.change(box, { target: { value: text } }); fireEvent.blur(box);
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith("p", expect.objectContaining({
    shotId: "explicit-size", prompt: text, selectionRef: "sel",
    values: expect.objectContaining({ width: { type: "integer", value: width }, height: { type: "integer", value: height } }),
  })));
  expect(api.generate).not.toHaveBeenCalled();
});

it('row generation renews the key only after acceptance or changed inputs, not unknown IPC outcomes', async () => {
  render(<StoryboardShotRows {...props()} items={[shot('keys',0)]}/>);
  const button=await screen.findByRole('button',{name:'生成视频'});
  fireEvent.click(button);
  await screen.findByText('已写入生产队列');
  fireEvent.click(button);
  await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
  const [first,second]=api.generate.mock.calls.map(c=>c[0].submissionIdempotencyKey);
  expect(first).not.toBe(second);
});

it('row generation changes the attempt key when the actual draft changes after an unknown outcome', async () => {
  api.generate.mockRejectedValueOnce({code:'INTERNAL_ERROR'});
  const p=props(); const item=shot('changing',0);
  const h=render(<StoryboardShotRows {...p} items={[item]}/>);
  fireEvent.click(await screen.findByRole('button',{name:'生成视频'}));
  await screen.findByRole('alert');
  h.rerender(<StoryboardShotRows {...p} items={[{...item,shot:{...item.shot!,values:{duration_seconds:{type:'integer',value:6}}}}]}/>);
  fireEvent.click(screen.getByRole('button',{name:'生成视频'}));
  await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
  expect(api.generate.mock.calls[1][0].submissionIdempotencyKey).not.toBe(api.generate.mock.calls[0][0].submissionIdempotencyKey);
});

it('row entity references block the old I2V selection even when the project default is now REF2VA', async () => {
  const old = { ...generator, mode: 'I2V' };
  const ref = { ...generator, selectionRef: 'ref', mode: 'REF2VA', recommended: true,
    fields: [...generator.fields, { key: 'reference_images', type: 'images', label: '参考图', required: false, maxItems: 9 }] };
  api.generatorsList.mockResolvedValue([ref, old]);
  const item = shot('entity-old', 0); item.shot!.prompt = '@阿离 在 @雨巷 行走';
  render(<StoryboardShotRows {...props()} items={[item]} />);
  await screen.findByTitle('解除绑定 角色 · 阿离');
  const generate = screen.getByRole('button', { name: '生成视频' });
  expect(generate.hasAttribute('disabled')).toBe(true);
  expect((screen.getByLabelText('镜头1 生成器') as HTMLSelectElement).value).toBe('sel');
  expect(screen.getByLabelText('角色与场景生成引用').textContent).toContain('REF2VA');
  fireEvent.click(generate);
  expect(api.readinessGet).not.toHaveBeenCalled();
  expect(api.generate).not.toHaveBeenCalled();
  expect(api.updateShot).not.toHaveBeenCalled();
});

const refGenerator = { ...generator, selectionRef: 'ref', mode: 'REF2VA', persistentInputs: true,
  fields: [...generator.fields, { key: 'reference_images', type: 'images', label: '参考图', required: false, maxItems: 9 }] };
function referenceShot() { const item = shot('entity-ref', 0, 'ref'); item.shot!.prompt = '@阿离 在 @雨巷 行走'; return item; }
function photos() {
  api.list.mockResolvedValue([{ ...hero, imageAssetIds: ['hero-image'] }, { ...lane, imageAssetIds: ['scene-image'] }]);
  api.generatorsList.mockResolvedValue([refGenerator, generator]);
}

it('row entity references save the explicitly chosen exact generator without creating media or GPU work', async () => {
  photos();
  const item = shot('entity-switch', 0); item.shot!.prompt = '@阿离 在 @雨巷 行走';
  const p = props(); const h = render(<StoryboardShotRows {...p} items={[item]} />);
  const select = await screen.findByLabelText('镜头1 生成器');
  await waitFor(() => expect(within(select).getAllByRole('option').length).toBe(2));
  fireEvent.change(select, { target: { value: 'ref' } });
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith('p', { shotId: item.id, name: item.name, stage: 'video', selectionRef: 'ref',
    values: expect.objectContaining({ duration_seconds: { type: 'integer', value: 5 } }) }));
  await screen.findByText(/镜头生成器已保存/);
  expect(screen.getByRole('button', { name: '生成视频' }).hasAttribute('disabled')).toBe(true);
  expect(api.inputsSave).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
  h.rerender(<StoryboardShotRows {...p} items={[{ ...item, shot: { ...item.shot!, selectionRef: 'ref' } }]} />);
  await waitFor(() => expect((screen.getByLabelText('镜头1 生成器') as HTMLSelectElement).value).toBe('ref'));
  await waitFor(() => expect(api.inputsGet).toHaveBeenCalledWith({ projectId: 'p', shotId: item.id, selectionRef: 'ref' }));
});

it('row entity references apply ordered images through OCC, retain audio, and generate only on a separate click', async () => {
  photos();
  const existing = [{ inputKey: 'reference_images', ordinal: 0, assetId: 'scene-image' }, { inputKey: 'reference_audios', ordinal: 0, assetId: 'audio' }];
  api.inputsGet.mockResolvedValue({ selectionRef: 'ref', token: { instanceId: 'refs', revision: 4 }, inputs: existing, assets: [] });
  render(<StoryboardShotRows {...props()} items={[referenceShot()]} />);
  const apply = await screen.findByRole('button', { name: '应用角色与场景参考图' });
  await waitFor(() => expect(apply.hasAttribute('disabled')).toBe(false));
  expect(screen.getByRole('button', { name: '生成视频' }).hasAttribute('disabled')).toBe(true);
  fireEvent.click(apply);
  await screen.findByText('角色与场景参考图已保存；尚未创建生成任务。');
  expect(api.inputsSave).toHaveBeenCalledWith({ selection: { projectId: 'p', shotId: 'entity-ref', selectionRef: 'ref' }, expected: { instanceId: 'refs', revision: 4 }, inputs: [
    existing[1], existing[0], { inputKey: 'reference_images', ordinal: 1, assetId: 'hero-image' },
  ] });
  const panel = screen.getByLabelText('角色与场景生成引用');
  expect(panel.textContent).toContain('@阿离 → <Picture 2>'); expect(panel.textContent).toContain('@雨巷 → <Picture 1>');
  expect(api.readinessGet).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole('button', { name: '生成视频' }));
  await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(1));
  expect(api.generate.mock.calls[0][0]).toMatchObject({ selectionRef: 'ref', shotId: 'entity-ref', values: { prompt: { type: 'string', value: '@阿离 在 @雨巷 行走' } } });
});

it('row entity references refuse OCC conflicts until an explicit fresh read and never auto-submit', async () => {
  photos(); api.inputsSave.mockRejectedValueOnce({ code: 'SHOT_VIDEO_INPUT_CONFLICT', details: {} });
  render(<StoryboardShotRows {...props()} items={[referenceShot()]} />);
  const apply = await screen.findByRole('button', { name: '应用角色与场景参考图' });
  await waitFor(() => expect(apply.hasAttribute('disabled')).toBe(false)); fireEvent.click(apply);
  await screen.findByRole('alert');
  expect(apply.hasAttribute('disabled')).toBe(true); expect(screen.getByRole('button', { name: '生成视频' }).hasAttribute('disabled')).toBe(true);
  expect(api.generate).not.toHaveBeenCalled();
  api.inputsGet.mockResolvedValueOnce({ selectionRef: 'ref', token: { instanceId: 'refs', revision: 9 }, inputs: [], assets: [] });
  fireEvent.click(screen.getByRole('button', { name: '重新读取参考输入' }));
  await waitFor(() => expect(apply.hasAttribute('disabled')).toBe(false)); fireEvent.click(apply);
  await waitFor(() => expect(api.inputsSave).toHaveBeenLastCalledWith(expect.objectContaining({ expected: { instanceId: 'refs', revision: 9 } })));
  expect(api.generate).not.toHaveBeenCalled();
});

it.each(['missing', 'unknown', 'foreign', 'ambiguous', 'cap'])('row entity references reject %s images before any save or generation', async kind => {
  photos(); const item = referenceShot();
  if (kind === 'missing') api.list.mockResolvedValue([hero, lane]);
  if (kind === 'unknown') item.shot!.prompt = '@不存在';
  if (kind === 'foreign') api.list.mockResolvedValue([{ ...hero, projectId: 'other', imageAssetIds: ['foreign-image'] }]);
  if (kind === 'ambiguous') api.list.mockResolvedValue([{ ...hero, imageAssetIds: ['hero-image'] }, { ...hero, id: 'duplicate', imageAssetIds: ['other-image'] }, lane]);
  if (kind === 'cap') api.generatorsList.mockResolvedValue([{ ...refGenerator, fields: [{ key: 'reference_images', type: 'images', maxItems: 1 }] }]);
  render(<StoryboardShotRows {...props()} items={[item]} />);
  if (kind === 'foreign') await waitFor(() => expect(screen.getByLabelText('角色与场景生成引用').textContent).toContain('无法识别'));
  else await screen.findByTitle('解除绑定 角色 · 阿离');
  await waitFor(() => expect(screen.getByRole('button', { name: '应用角色与场景参考图' }).hasAttribute('disabled')).toBe(true));
  expect(screen.getByRole('button', { name: '生成视频' }).hasAttribute('disabled')).toBe(true);
  expect(api.inputsSave).not.toHaveBeenCalled(); expect(api.readinessGet).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
});

it('row entity references ignore a late prior-project input read', async () => {
  photos(); const delayed = deferred<unknown>(); api.inputsGet.mockReturnValueOnce(delayed.promise);
  const p = props(), item = referenceShot(); const h = render(<StoryboardShotRows {...p} items={[item]} />);
  await waitFor(() => expect(api.inputsGet).toHaveBeenCalledTimes(1));
  h.rerender(<StoryboardShotRows {...p} projectId='other' items={[item]} />);
  await act(async () => delayed.resolve({ selectionRef: 'ref', token: { instanceId: 'old', revision: 9 }, inputs: [{ inputKey: 'reference_images', ordinal: 0, assetId: 'hero-image' }], assets: [] }));
  expect(screen.getByRole('button', { name: '生成视频' }).hasAttribute('disabled')).toBe(true);
  expect(screen.getByLabelText('角色与场景生成引用').textContent).not.toContain('<Picture 1>');
  expect(api.inputsSave).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
});

it('keeps the typed backend entity-reference error actionable instead of hiding it as INTERNAL_ERROR', () => {
  const error = normalizeProductError({ code: 'ENTITY_REFERENCE_INVALID', details: { field: 'prompt', action: 'EDIT_INPUT', retryable: false, technicalDetails: 'ENTITY_REFERENCE_MODE' } });
  expect(error.code).toBe('ENTITY_REFERENCE_INVALID'); expect(error.message).toContain('当前镜头');
  expect(error.details.field).toBe('prompt'); expect(error.details.technicalDetails).toBe('ENTITY_REFERENCE_MODE');
});

it('row entity references do not dispatch a late READY after formal reference inputs change', async () => {
  photos(); const item = referenceShot();
  const inputs = [{ inputKey: 'reference_images', ordinal: 0, assetId: 'hero-image' }, { inputKey: 'reference_images', ordinal: 1, assetId: 'scene-image' }];
  api.inputsGet.mockResolvedValue({ selectionRef: 'ref', token: { instanceId: 'refs', revision: 4 }, inputs, assets: [] });
  const ready = deferred<{ ready: boolean; issues: []; fieldErrors: []; actions: [] }>(); api.readinessGet.mockReturnValueOnce(ready.promise);
  render(<StoryboardShotRows {...props()} items={[item]} />);
  const generate = screen.getByRole('button', { name: '生成视频' });
  await waitFor(() => expect(generate.hasAttribute('disabled')).toBe(false)); fireEvent.click(generate);
  await waitFor(() => expect(api.readinessGet).toHaveBeenCalledTimes(1));
  api.inputsGet.mockResolvedValueOnce({ selectionRef: 'ref', token: { instanceId: 'refs', revision: 5 }, inputs: [], assets: [] });
  act(() => invalidateVideoInputs('p'));
  await waitFor(() => expect(screen.getByLabelText('角色与场景生成引用').textContent).toContain('尚未进入生成输入'));
  await act(async () => ready.resolve({ ready: true, issues: [], fieldErrors: [], actions: [] }));
  expect(api.generate).not.toHaveBeenCalled();
});

it('row entity references ignore a save receipt after project switch without touching the new project', async () => {
  photos(); const pending = deferred<unknown>(); api.inputsSave.mockReturnValueOnce(pending.promise);
  const p = props(), item = referenceShot(); const h = render(<StoryboardShotRows {...p} items={[item]} />);
  const apply = screen.getByRole('button', { name: '应用角色与场景参考图' });
  await waitFor(() => expect(apply.hasAttribute('disabled')).toBe(false)); fireEvent.click(apply);
  await waitFor(() => expect(api.inputsSave).toHaveBeenCalledTimes(1));
  h.rerender(<StoryboardShotRows {...p} projectId='other' items={[item]} />);
  await act(async () => pending.resolve({ selectionRef: 'ref', token: { instanceId: 'old', revision: 5 }, inputs: [], assets: [] }));
  expect(screen.queryByText('角色与场景参考图已保存；尚未创建生成任务。')).toBeNull();
  expect(api.inputsSave.mock.calls[0][0].selection.projectId).toBe('p'); expect(api.inputsSave).toHaveBeenCalledTimes(1);
  expect(p.onChanged).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
});

it('row entity references keep a failed generator switch on the old exact pair', async () => {
  photos(); const item = shot('switch-fail', 0); item.shot!.prompt = '@阿离'; api.updateShot.mockRejectedValueOnce({ code: 'INVALID_INPUT' });
  render(<StoryboardShotRows {...props()} items={[item]} />);
  const select = screen.getByLabelText('镜头1 生成器');
  await waitFor(() => expect(within(select).getAllByRole('option').length).toBe(2)); fireEvent.change(select, { target: { value: 'ref' } });
  await screen.findByRole('alert');
  expect((select as HTMLSelectElement).value).toBe('sel'); expect(select.hasAttribute('disabled')).toBe(false);
  expect(api.inputsSave).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
});

it('plain T2V row generation needs no image or entity-reference save', async () => {
  render(<StoryboardShotRows {...props()} items={[shot('plain', 0)]} />);
  fireEvent.click(await screen.findByRole('button', { name: '生成视频' }));
  await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(1));
  expect(api.inputsGet).not.toHaveBeenCalled(); expect(api.inputsSave).not.toHaveBeenCalled();
});
