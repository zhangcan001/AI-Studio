import { useStudioStore } from "../../stores/studioStore";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";
import { StoryboardShotRows } from "./StoryboardShotRows";
import type { StoryboardShot } from "./storyboardModel";
import { normalizeProductError } from "../../product/errors";
import { invalidateVideoInputs } from "../../product/videoInputInvalidation";

const api = vi.hoisted(() => ({ saveShotRow: vi.fn(), updateShot: vi.fn(), deleteShot: vi.fn(), readinessGet: vi.fn(), generate: vi.fn(), selectResult: vi.fn(), list: vi.fn(), listForShot: vi.fn(), bindShot: vi.fn(), generatorsList: vi.fn(), inputsGet: vi.fn(), inputsSave: vi.fn(), compositionUsage: vi.fn(), setCompositionUsage: vi.fn(), promptSections: vi.fn(), setPromptSections: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: {
  creation: { saveShotRow: api.saveShotRow, updateShot: api.updateShot, deleteShot: api.deleteShot, readinessGet: api.readinessGet, generate: api.generate, selectResult: api.selectResult, generatorsList: api.generatorsList, inputsGet: api.inputsGet, inputsSave: api.inputsSave, mediaUrl: (p: string, id: string) => `media://${p}/${id}` },
  storyEntities: { list: api.list, listForShot: api.listForShot, bindShot: api.bindShot, compositionUsage: api.compositionUsage, setCompositionUsage: api.setCompositionUsage, promptSections: api.promptSections, setPromptSections: api.setPromptSections },
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
  sessionStorage.clear();useStudioStore.setState({storyboardPrompts:{}});
  api.list.mockResolvedValue([hero, lane]); api.listForShot.mockResolvedValue([hero]);
  api.bindShot.mockImplementation(async (_p: string, _s: string, ids: string[]) => [hero, lane].filter(e => ids.includes(e.id)));
  api.readinessGet.mockResolvedValue({ ready: true, issues: [], fieldErrors: [], actions: [] });
  api.generate.mockResolvedValue({ accepted: true, runRef: { source: "queue-batch", id: "r" }, startOutcome: "QUEUED", startIssue: null });
  api.generatorsList.mockResolvedValue([generator]);
  api.inputsGet.mockImplementation(async s => ({ selectionRef: s.selectionRef, token: { instanceId: 'refs', revision: 4 }, inputs: [], assets: [] }));
  api.inputsSave.mockImplementation(async r => ({ selectionRef: r.selection.selectionRef, token: { instanceId: 'refs', revision: 5 }, inputs: r.inputs, assets: [] }));
  api.saveShotRow.mockImplementation(async (_p: string, req: {shotId:string;prompt?:string;values?:Record<string,unknown>;selectionRef?:string;entityIds?: string[]}) => ({ shot: {...shot(req.shotId,0).shot!,...(req.prompt !== undefined ? {prompt:req.prompt}:{}),...(req.values ? {values:req.values}:{}),...(req.selectionRef ? {selectionRef:req.selectionRef}:{})}, boundEntities: [hero, lane].filter(e => (req.entityIds ?? ["e1"]).includes(e.id)) }));
  api.updateShot.mockResolvedValue({}); api.deleteShot.mockResolvedValue(undefined);
  api.compositionUsage.mockResolvedValue({ openingEntityId: null, endingEntityId: null });
  api.promptSections.mockResolvedValue({ style: null, summary: null, soundscape: null, music: null });
  api.setPromptSections.mockImplementation(async (_p: string, _s: string, sections: unknown) => sections);
  api.setCompositionUsage.mockImplementation(async (_p: string, _s: string, usage: unknown) => usage);
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
  const button = await screen.findByRole("button", { name: "生成视频" });
  await waitFor(() => expect(button.hasAttribute("disabled")).toBe(false));
  fireEvent.click(button);
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
  expect(within(row).getByLabelText("提示词写作建议").textContent).toContain("镜头描述建议用英文书写（对白可保留中文）");
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
  await waitFor(() => expect(within(first).getByRole("button", { name: "生成视频" }).hasAttribute("disabled")).toBe(false));
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
  await waitFor(() => expect(api.saveShotRow).toHaveBeenCalledWith("p", { shotId: "a", stage: "video", prompt: "阿离 走进 雨巷 8秒 竖屏 720p", selectionRef: "sel",
    values: { duration_seconds: { type: "integer", value: 8 }, width: { type: "integer", value: 768 }, height: { type: "integer", value: 1344 } }, entityIds: ["e1", "e2"] }));
  expect(api.bindShot).not.toHaveBeenCalled();
  expect(api.updateShot).not.toHaveBeenCalled();
  expect(api.saveShotRow).toHaveBeenCalledTimes(1);
  const status = (await within(row).findByText(/已识别并填入/)).closest("p")!;
  expect(status.textContent).toContain("已识别并填入：时长 8 秒 · 画幅 9:16 · 768×1344 · 场景 @雨巷");
  expect(p.onChanged).toHaveBeenCalled();
  fireEvent.click(within(status).getByRole("button", { name: "撤销" }));
  await waitFor(() => expect(api.saveShotRow).toHaveBeenLastCalledWith("p", { shotId: "a", stage: "video", selectionRef: "sel", values: { duration_seconds: { type: "integer", value: 5 } }, entityIds: ["e1"] }));
  expect(api.saveShotRow).toHaveBeenCalledTimes(2);
  expect(api.bindShot).not.toHaveBeenCalled();
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
  await waitFor(() => expect(api.saveShotRow).toHaveBeenLastCalledWith("p", { shotId: "a", stage: "video", prompt: "只改文字" }));
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

it("uses available REF2VA for new unconfigured rows but preserves an existing FIRST_LAST row", async () => {
  const firstLast = { ...generator, selectionRef: "fl-default", mode: "FIRST_LAST" };
  const reference = { ...generator, selectionRef: "ref-default", mode: "REF2VA", recommended: false };
  api.generatorsList.mockResolvedValue([firstLast, reference]);
  const existing = shot("existing", 1);
  existing.shot = { ...existing.shot!, selectionRef: firstLast.selectionRef };
  render(<StoryboardShotRows {...props()} items={[shot("new", 0, null), existing]} />);
  const rows = await screen.findAllByRole("article");
  for (const [index, row] of rows.entries()) {
    const control = await within(row).findByLabelText(`镜头${index + 1} 画幅`);
    fireEvent.change(control, { target: { value: "9:16" } });
  }
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith("p", expect.objectContaining({ shotId: "new", selectionRef: reference.selectionRef })));
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith("p", expect.objectContaining({ shotId: "existing", selectionRef: firstLast.selectionRef })));
  expect(api.generate).not.toHaveBeenCalled();
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
  await waitFor(() => expect(api.saveShotRow).toHaveBeenCalledWith("p", { shotId: "b", stage: "video", prompt: "竖屏 6秒" }));
});

it("explains an incomplete width-only setting instead of silently ignoring it", async () => {
  render(<StoryboardShotRows {...props()} items={[shot("width-only", 0)]} />);
  const box = await screen.findByLabelText("镜头1 提示词");
  await waitFor(() => expect(box.hasAttribute("disabled")).toBe(false));
  fireEvent.change(box, { target: { value: "画幅1920" } });
  expect(screen.getByRole("status", { name: "提示词识别提示" }).textContent).toContain("画幅/分辨率信息不完整");
  fireEvent.blur(box);
  await waitFor(() => expect(api.saveShotRow).toHaveBeenCalledWith("p", { shotId: "width-only", stage: "video", prompt: "画幅1920" }));
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
  await waitFor(() => expect(api.saveShotRow).toHaveBeenCalledWith("p", expect.objectContaining({
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
  await waitFor(() => expect(api.saveShotRow).toHaveBeenCalledWith("p", expect.objectContaining({
    shotId: "explicit-size", prompt: text, selectionRef: "sel",
    values: expect.objectContaining({ width: { type: "integer", value: width }, height: { type: "integer", value: height } }),
  })));
  expect(api.generate).not.toHaveBeenCalled();
});

it('row generation renews the key only after acceptance or changed inputs, not unknown IPC outcomes', async () => {
  render(<StoryboardShotRows {...props()} items={[shot('keys',0)]}/>);
  const button=await screen.findByRole('button',{name:'生成视频'});
  await waitFor(()=>expect(button.hasAttribute('disabled')).toBe(false));
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
  const button=await screen.findByRole('button',{name:'生成视频'});
  await waitFor(()=>expect(button.hasAttribute('disabled')).toBe(false));fireEvent.click(button);
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

it('row entity references automatically apply ordered images through OCC, retain audio, and generate only on a separate click', async () => {
  photos();
  const existing = [{ inputKey: 'reference_images', ordinal: 0, assetId: 'scene-image' }, { inputKey: 'reference_audios', ordinal: 0, assetId: 'audio' }];
  api.inputsGet.mockResolvedValue({ selectionRef: 'ref', token: { instanceId: 'refs', revision: 4 }, inputs: existing, assets: [] });
  render(<StoryboardShotRows {...props()} items={[referenceShot()]} />);
  await screen.findByRole('button', { name: '应用角色与场景参考图' });
  expect(screen.getByRole('button', { name: '生成视频' }).hasAttribute('disabled')).toBe(true);
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
  await waitFor(() => expect(screen.getByLabelText('角色与场景生成引用').textContent).toContain('正在自动应用'));
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
  const button=await screen.findByRole('button', { name: '生成视频' });
  await waitFor(()=>expect(button.hasAttribute('disabled')).toBe(false));fireEvent.click(button);
  await waitFor(() => expect(api.generate).toHaveBeenCalledTimes(1));
  expect(api.inputsGet).not.toHaveBeenCalled(); expect(api.inputsSave).not.toHaveBeenCalled();
});

it('shows an alert and retry when story entity list fails, and recovers on retry', async () => {
  const cause = { code: 'PROJECT_READ_FAILED' };
  api.list.mockRejectedValueOnce(cause);
  const item = shot('failed-entities', 0); item.shot!.prompt = '@阿离';
  render(<StoryboardShotRows {...props()} items={[item]} />);
  const alert = await screen.findByRole('alert');
  expect(alert.textContent).toContain(normalizeProductError(cause).message);
  expect(screen.queryByText(/正在读取当前项目角色与场景/)).toBeNull();
  expect(screen.queryByText('素材库中还没有角色与场景')).toBeNull();
  fireEvent.click(within(alert).getByRole('button', { name: '重试角色与场景' }));
  await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
  expect(await screen.findByTitle('绑定 场景 · 雨巷')).toBeTruthy();
  expect(api.generate).not.toHaveBeenCalled();
});

it('shows an alert and retry when generator list fails, and recovers on retry', async () => {
  const cause = { code: 'GENERATOR_UNAVAILABLE' };
  api.generatorsList.mockRejectedValueOnce(cause);
  const item = shot('failed-generators', 0); item.shot!.prompt = '@阿离';
  render(<StoryboardShotRows {...props()} items={[item]} />);
  const alert = await screen.findByRole('alert');
  expect(alert.textContent).toContain(normalizeProductError(cause).message);
  expect(screen.queryByText(/当前镜头仍使用 未设置/)).toBeNull();
  fireEvent.click(within(alert).getByRole('button', { name: '重试生成器' }));
  await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
  expect(within(screen.getByLabelText('镜头1 生成器')).getByRole('option', { name: /H3/ })).toBeTruthy();
});

it('clears list errors when switching project', async () => {
  api.list.mockRejectedValueOnce({ code: 'PROJECT_READ_FAILED' });
  api.generatorsList.mockRejectedValueOnce({ code: 'GENERATOR_UNAVAILABLE' });
  const p = props(); const h = render(<StoryboardShotRows {...p} items={[shot('a', 0)]} />);
  await waitFor(() => expect(screen.getAllByRole('alert')).toHaveLength(2));
  h.rerender(<StoryboardShotRows {...p} projectId='other' items={[shot('b', 0)]} />);
  expect(screen.queryAllByRole('alert')).toHaveLength(0);
  await waitFor(() => expect(api.list).toHaveBeenLastCalledWith('other', null));
  expect(screen.queryAllByRole('alert')).toHaveLength(0);
});

it('ignores a late failed list response from the previous project', async () => {
  let reject!: (cause: unknown) => void;
  api.list.mockReturnValueOnce(new Promise((_, no) => { reject = no; }));
  const p = props(); const h = render(<StoryboardShotRows {...p} items={[shot('a', 0)]} />);
  await waitFor(() => expect(api.list).toHaveBeenCalledWith('p', null));
  h.rerender(<StoryboardShotRows {...p} projectId='other' items={[shot('b', 0)]} />);
  await waitFor(() => expect(api.list).toHaveBeenLastCalledWith('other', null));
  await act(async () => reject({ code: 'PROJECT_READ_FAILED' }));
  expect(screen.queryAllByRole('alert')).toHaveLength(0);
});

it("preserves prompt draft and offers no undo when the atomic save fails", async () => {
  api.saveShotRow.mockRejectedValueOnce({code: "INVALID_INPUT"});
  const p = props(); render(<StoryboardShotRows {...p} items={[shot("a",0)]}/>);
  const box = await screen.findByLabelText("镜头1 提示词");
  await waitFor(() => expect(box.hasAttribute("disabled")).toBe(false));
  fireEvent.change(box, {target:{value:"阿离 雨巷 8秒"}}); fireEvent.blur(box);
  await waitFor(() => expect(api.saveShotRow).toHaveBeenCalledTimes(1));
  await screen.findByRole("alert");
  expect((box as HTMLTextAreaElement).value).toBe("阿离 雨巷 8秒");
  expect(screen.queryByRole("button", {name:"撤销"})).toBeNull();
  expect(api.updateShot).not.toHaveBeenCalled(); expect(api.bindShot).not.toHaveBeenCalled(); expect(p.onChanged).not.toHaveBeenCalled();
});

it('automatically applies newly saved row mentions once, without saving an unfinished edit', async () => {
  photos(); const p = props(), item = referenceShot(); item.shot!.prompt = '';
  const h = render(<StoryboardShotRows {...p} items={[item]} />);
  const editor = screen.getByLabelText('镜头1 提示词');
  await waitFor(() => expect(editor.hasAttribute('disabled')).toBe(false));
  fireEvent.change(editor, { target: { value: '@阿离' } });
  await act(async () => { await new Promise(r => setTimeout(r, 320)); });
  expect(api.inputsSave).not.toHaveBeenCalled();
  fireEvent.blur(editor);
  await waitFor(() => expect(api.saveShotRow).toHaveBeenCalled());
  h.rerender(<StoryboardShotRows {...p} items={[{ ...item, shot: { ...item.shot!, prompt: '@阿离' } }]} />);
  await waitFor(() => expect(api.inputsSave).toHaveBeenCalledTimes(1));
  await act(async () => { await new Promise(r => setTimeout(r, 400)); });
  expect(api.inputsSave).toHaveBeenCalledTimes(1);
  expect(api.inputsSave.mock.calls[0][0].inputs).toEqual([{ inputKey: 'reference_images', ordinal: 0, assetId: 'hero-image' }]);
  expect(api.generate).not.toHaveBeenCalled();
});

it('cancels a pending automatic row reference write on project switch', async () => {
  photos(); const p = props(), item = referenceShot();
  const h = render(<StoryboardShotRows {...p} items={[item]} />);
  await waitFor(() => expect(api.inputsGet).toHaveBeenCalledTimes(1));
  h.rerender(<StoryboardShotRows {...p} projectId='other' items={[item]} />);
  await act(async () => { await new Promise(r => setTimeout(r, 400)); });
  expect(api.inputsSave).not.toHaveBeenCalled(); expect(api.generate).not.toHaveBeenCalled();
});

it("chooses 构图用途 only among the shot's bound compositions and keeps opening and ending distinct", async () => {
  const open = { ...hero, id: "c1", kind: "composition", name: "取账前过渡" } as const;
  const end = { ...hero, id: "c2", kind: "composition", name: "取账尾" } as const;
  api.list.mockResolvedValue([hero, open, end]);
  api.listForShot.mockResolvedValue([hero, open, end]);
  api.compositionUsage.mockResolvedValue({ openingEntityId: "c1", endingEntityId: null });
  render(<StoryboardShotRows {...props()} items={[shot("a", 0)]} />);
  const opening = await screen.findByLabelText("镜头1 开场构图") as HTMLSelectElement;
  const ending = screen.getByLabelText("镜头1 结尾构图") as HTMLSelectElement;
  await waitFor(() => expect(opening.value).toBe("c1"));
  expect([...opening.options].map(o => o.textContent)).toEqual(["不指定", "取账前过渡", "取账尾"]);
  expect([...ending.options].find(o => o.value === "c1")?.disabled).toBe(true);
  await waitFor(() => expect(ending.hasAttribute("disabled")).toBe(false));
  fireEvent.change(ending, { target: { value: "c2" } });
  await waitFor(() => expect(api.setCompositionUsage).toHaveBeenCalledWith("p", "a", { openingEntityId: "c1", endingEntityId: "c2" }));
  await waitFor(() => expect(ending.value).toBe("c2"));
  expect(screen.queryByText(/角色用途/)).toBeNull();
});

it("hides 构图用途 when the shot has no bound composition", async () => {
  render(<StoryboardShotRows {...props()} items={[shot("a", 0)]} />);
  await screen.findByTitle("绑定 场景 · 雨巷");
  expect(screen.queryByLabelText("镜头1 开场构图")).toBeNull();
});

it("blocks over-limit (11 s) shots: existing value kept, error shown, generation disabled, longer edits rejected", async () => {
  const long = shot("a", 0); long.shot!.values = { duration_seconds: { type: "integer", value: 14 } };
  render(<StoryboardShotRows {...props()} items={[long]} />);
  const row = await screen.findByRole("article", { name: "分镜 1 镜头1" });
  const input = await within(row).findByLabelText("镜头1 时长（秒）") as HTMLInputElement;
  expect(input.value).toBe("14");
  expect(input.getAttribute("max")).toBe("10");
  expect(within(row).getAllByText(/镜头时长最长 11 秒（显存限制）/).length).toBeGreaterThan(0);
  expect((within(row).getByRole("button", { name: "生成视频" }) as HTMLButtonElement).disabled).toBe(true);
  fireEvent.change(input, { target: { value: "13" } }); fireEvent.blur(input);
  await new Promise(r => setTimeout(r, 20));
  expect(api.updateShot).not.toHaveBeenCalled();
});

it("waits for prompt sections before saving and merges a late read without overwriting edited fields", async () => {
  const pending = deferred<{ style: string; summary: string; soundscape: string; music: string }>();
  api.promptSections.mockReturnValueOnce(pending.promise);
  render(<StoryboardShotRows {...props()} items={[shot("a", 0)]} />);
  const style = screen.getByLabelText("镜头1 风格");
  await waitFor(() => expect(style.hasAttribute("disabled")).toBe(false));
  fireEvent.change(style, { target: { value: "new style" } }); fireEvent.blur(style);
  expect(api.setPromptSections).not.toHaveBeenCalled();
  await act(async () => pending.resolve({ style: "old style", summary: "saved summary", soundscape: "saved sound", music: "saved music" }));
  expect((style as HTMLTextAreaElement).value).toBe("new style");
  expect((screen.getByLabelText("镜头1 摘要") as HTMLTextAreaElement).value).toBe("saved summary");
  fireEvent.blur(style);
  await waitFor(() => expect(api.setPromptSections).toHaveBeenCalledWith("p", "a", { style: "new style", summary: "saved summary", soundscape: "saved sound", music: "saved music" }));
});

it("shows a Chinese prompt-section read error and blocks saves after a failed read", async () => {
  api.promptSections.mockRejectedValueOnce({ code: "PROJECT_READ_FAILED" });
  render(<StoryboardShotRows {...props()} items={[shot("a", 0)]} />);
  expect((await screen.findByRole("alert")).textContent).toContain("提示词分段读取失败");
  const style = screen.getByLabelText("镜头1 风格");
  await waitFor(() => expect(style.hasAttribute("disabled")).toBe(false));
  fireEvent.change(style, { target: { value: "new style" } }); fireEvent.blur(style);
  expect(api.setPromptSections).not.toHaveBeenCalled();
});

it("ignores a late prompt-section read after switching projects", async () => {
  const pending = deferred<{ style: string; summary: null; soundscape: null; music: null }>();
  api.promptSections.mockReturnValueOnce(pending.promise);
  const p = props(); const view = render(<StoryboardShotRows {...p} items={[shot("a", 0)]} />);
  await waitFor(() => expect(api.promptSections).toHaveBeenCalledWith("p", "a"));
  view.rerender(<StoryboardShotRows {...p} projectId="other" items={[shot("a", 0)]} />);
  await waitFor(() => expect(api.promptSections).toHaveBeenCalledWith("other", "a"));
  await act(async () => pending.resolve({ style: "old project", summary: null, soundscape: null, music: null }));
  expect((screen.getByLabelText("镜头1 风格") as HTMLTextAreaElement).value).toBe("");
});

it('changed saved prompt sections after a lost acknowledgement must renew row intent identity',async()=>{
 api.generatorsList.mockResolvedValue([{...generator,mode:'REF2VA',persistentInputs:true,fields:[...generator.fields,{type:'images',key:'reference_images',label:'Images',required:false,minItems:0,maxItems:8}]}]);
 api.setPromptSections.mockImplementation(async (_p,_s,next)=>{api.promptSections.mockResolvedValue(next);return next;});
 const p=props();render(<StoryboardShotRows {...p} items={[shot('a',0)]}/>);
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 await waitFor(()=>expect(api.promptSections).toHaveBeenCalled());
 api.generate.mockRejectedValueOnce(new Error('ack lost after persistence'));
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 const summary=screen.getByLabelText('镜头1 摘要');
 fireEvent.change(summary,{target:{value:'A very different scene in a flooded city.'}});fireEvent.blur(summary);
 await waitFor(()=>expect(api.setPromptSections).toHaveBeenCalledWith('p','a',expect.objectContaining({summary:'A very different scene in a flooded city.'})));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
 const [a,b]=api.generate.mock.calls.map(x=>x[0]);
 expect(b.submissionIdempotencyKey,'changed formal prompt sections must not replay the earlier frozen batch').not.toBe(a.submissionIdempotencyKey);
});
it('no-mention row must observe a formal media revision before retrying an uncertain submission',async()=>{
 api.generatorsList.mockResolvedValue([{...generator,mode:'I2V',persistentInputs:true,fields:[...generator.fields,{type:'image',key:'first_frame',label:'First',required:true}]}]);
 render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 api.generate.mockRejectedValueOnce(new Error('ack lost after persistence'));
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 api.inputsGet.mockResolvedValue({selectionRef:'sel',token:{instanceId:'inputs',revision:9},inputs:[{inputKey:'first_frame',ordinal:0,assetId:'changed-image'}],assets:[]});
 await act(async()=>{invalidateVideoInputs('p');});
 // Allow the real notification/render effect chain to run; no retry timer or simulated GPU.
 await act(async()=>{await new Promise(r=>setTimeout(r,30));});
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
 const [a,b]=api.generate.mock.calls.map(x=>x[0]);
 expect(b.submissionIdempotencyKey,'current row cannot reuse frozen media after a known formal-input invalidation').not.toBe(a.submissionIdempotencyKey);
});
it('unchanged row retries preserve unknown-outcome identity',async()=>{
 render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 api.generate.mockRejectedValueOnce(new Error('ack lost'));
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
 expect(api.generate.mock.calls[1][0].submissionIdempotencyKey).toBe(api.generate.mock.calls[0][0].submissionIdempotencyKey);
});

it.each(['I2V','FIRST_LAST','REF2VA'])('retains %s retry identity after unchanged formal evidence and invalidation',async mode=>{
 api.generatorsList.mockResolvedValue([{...generator,mode,persistentInputs:true}]);
 render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 api.generate.mockRejectedValueOnce(new Error('ack lost'));
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 await act(async()=>{invalidateVideoInputs('p');});
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
 expect(api.inputsGet).toHaveBeenCalledTimes(2);
 expect(api.generate.mock.calls[1][0].submissionIdempotencyKey).toBe(api.generate.mock.calls[0][0].submissionIdempotencyKey);
});
it('renews retry identity after persisted composition usage changes',async()=>{
 api.generatorsList.mockResolvedValue([{...generator,mode:'REF2VA',persistentInputs:true}]);
 render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 api.generate.mockRejectedValueOnce(new Error('ack lost'));
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 api.compositionUsage.mockResolvedValue({openingEntityId:'composition-new',endingEntityId:null});
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(2));
 expect(api.generate.mock.calls[1][0].submissionIdempotencyKey).not.toBe(api.generate.mock.calls[0][0].submissionIdempotencyKey);
});
it('rejects a mismatched fresh media identity before readiness or generation',async()=>{
 api.generatorsList.mockResolvedValue([{...generator,mode:'I2V',persistentInputs:true}]);
 api.inputsGet.mockResolvedValue({selectionRef:'other-recipe',token:null,inputs:[],assets:[]});
 render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));await screen.findByRole('alert');
 expect(api.readinessGet).not.toHaveBeenCalled();expect(api.generate).not.toHaveBeenCalled();
});
it('abandons a late formal evidence read after row/project replacement',async()=>{
 api.generatorsList.mockResolvedValue([{...generator,mode:'I2V',persistentInputs:true}]);
 const p=props(),read=deferred<{selectionRef:string;token:null;inputs:[];assets:[]}>();api.inputsGet.mockReturnValueOnce(read.promise);
 const host=render(<StoryboardShotRows {...p} items={[shot('a',0)]}/>);
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 fireEvent.click(screen.getByRole('button',{name:'生成视频'}));await waitFor(()=>expect(api.inputsGet).toHaveBeenCalled());
 host.rerender(<StoryboardShotRows {...p} projectId='other' items={[shot('b',0)]}/>);
 await act(async()=>{read.resolve({selectionRef:'sel',token:null,inputs:[],assets:[]});});
 expect(api.readinessGet).not.toHaveBeenCalled();expect(api.generate).not.toHaveBeenCalled();
});

it('unrelated wording edits must preserve manually chosen duration and aspect',async()=>{
 const item=shot('a',0);item.shot!.prompt='Walk for 8秒，竖屏';
 item.shot!.values={duration_seconds:{type:'integer',value:6},width:{type:'integer',value:1344},height:{type:'integer',value:768}};
 render(<StoryboardShotRows {...props()} items={[item]}/>);
 const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 fireEvent.change(box,{target:{value:'Walk for 8秒，竖屏，beneath soft light'}});fireEvent.blur(box);
 await waitFor(()=>expect(api.saveShotRow).toHaveBeenCalledTimes(1));
 const request=api.saveShotRow.mock.calls[0][1]; expect(request.values,'unchanged parameter phrases must not undo later explicit manual settings').toBeUndefined();
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
});
it('explicit changed parameter wording still applies the new duration',async()=>{
 const item=shot('a',0);item.shot!.prompt='Walk for 8秒';
 item.shot!.values={duration_seconds:{type:'integer',value:6}};
 render(<StoryboardShotRows {...props()} items={[item]}/>);
 const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 fireEvent.change(box,{target:{value:'Walk for 7秒'}});fireEvent.blur(box);
 await waitFor(()=>expect(api.saveShotRow).toHaveBeenCalledTimes(1));
 expect(api.saveShotRow.mock.calls[0][1].values.duration_seconds).toEqual({type:'integer',value:7});
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
});
it('one user click on Generate after editing must survive the asynchronous blur save',async()=>{
 let resolve!:(v:unknown)=>void;const save=new Promise(r=>resolve=r);api.saveShotRow.mockReturnValueOnce(save);
 const user=userEvent.setup();render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 await user.click(box);fireEvent.change(box,{target:{value:'Only new action text.'}});
 await user.click(screen.getByRole('button',{name:'生成视频'}));
 expect(api.saveShotRow).toHaveBeenCalledTimes(1);
 const disabledDuringSave=screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled');
 expect(disabledDuringSave).toBe(true);
 await act(async()=>resolve({shot:{...shot('a',0).shot,prompt:'Only new action text.'},boundEntities:[hero]}));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 await waitFor(()=>expect(api.generate,'a valid first click should not disappear after its required save').toHaveBeenCalledTimes(1));
});
it('a saved row generates on one normal user click',async()=>{
 const user=userEvent.setup();render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 await user.click(screen.getByRole('button',{name:'生成视频'}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 expect(api.saveShotRow).not.toHaveBeenCalled();
});
it('an accepted batch that failed to start must display a start failure rather than a queued success',async()=>{
 api.generate.mockResolvedValueOnce({accepted:true,runRef:{source:'queue-batch',id:'saved-not-started'},startOutcome:'FAILED_TO_START',startIssue:{code:'RUNTIME_BLOCKED',message:'运行环境暂时不可用',details:{action:'OPEN_RUNTIME_SETTINGS'}}});
 const user=userEvent.setup();render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 await user.click(screen.getByRole('button',{name:'生成视频'}));await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 expect(screen.queryByText('已写入生产队列'),'persisted acceptance is not a successfully queued/startable task').toBeNull();
 expect(screen.getByText(/启动失败|无法启动/)).toBeTruthy();
});

it('does not generate or preflight after a required prompt save fails',async()=>{
 const user=userEvent.setup();api.saveShotRow.mockRejectedValueOnce({code:'PROJECT_READ_FAILED',message:'save failed'});
 render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 await user.click(box);fireEvent.change(box,{target:{value:'Keep my unsaved text'}});
 await user.click(screen.getByRole('button',{name:'生成视频'}));await screen.findByRole('alert');
 expect((box as HTMLTextAreaElement).value).toBe('Keep my unsaved text');
 expect(api.saveShotRow).toHaveBeenCalledTimes(1);expect(api.readinessGet).not.toHaveBeenCalled();expect(api.generate).not.toHaveBeenCalled();
});
it('saves once then submits recognized values once even on a repeated click during save',async()=>{
 const user=userEvent.setup(),p=deferred<unknown>();api.saveShotRow.mockReturnValueOnce(p.promise);
 render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 await user.click(box);fireEvent.change(box,{target:{value:'Walk for 7秒'}});
 await user.click(screen.getByRole('button',{name:'生成视频'}));await user.click(screen.getByRole('button',{name:'生成视频'}));
 expect(api.readinessGet).not.toHaveBeenCalled();expect(api.generate).not.toHaveBeenCalled();
 const saved={...shot('a',0).shot!,prompt:'Walk for 7秒',values:{duration_seconds:{type:'integer' as const,value:7}}};
 await act(async()=>{p.resolve({shot:saved,boundEntities:[hero]});});
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));
 expect(api.saveShotRow).toHaveBeenCalledTimes(1);expect(api.generate.mock.calls[0][0].values.duration_seconds).toEqual({type:'integer',value:7});
});
it('allows its own accepted save refresh without cancelling the explicit generation',async()=>{
 const user=userEvent.setup();let saved=shot('a',0);let update!: (next:StoryboardShot)=>void;
 function Host(){const[item,setItem]=useState(saved);update=setItem;return <StoryboardShotRows {...props()} items={[item]} onChanged={()=>update(saved)}/>;}
 api.saveShotRow.mockImplementation(async(_p,req)=>{saved={...saved,shot:{...saved.shot!,prompt:req.prompt,values:req.values}};return{shot:saved.shot,boundEntities:[hero]};});
 render(<Host/>);const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 await user.click(box);fireEvent.change(box,{target:{value:'Walk for 7秒'}});await user.click(screen.getByRole('button',{name:'生成视频'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));expect(api.generate.mock.calls[0][0].values.duration_seconds.value).toBe(7);
});
it('aborts generation when a save acknowledgement arrives after switching project/row',async()=>{
 const user=userEvent.setup(),p=deferred<unknown>();api.saveShotRow.mockReturnValueOnce(p.promise);const callbacks=props();
 const view=render(<StoryboardShotRows {...callbacks} items={[shot('a',0)]}/>);const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 await user.click(box);fireEvent.change(box,{target:{value:'Do not generate after leaving'}});await user.click(screen.getByRole('button',{name:'生成视频'}));
 view.rerender(<StoryboardShotRows {...callbacks} projectId='other' items={[shot('b',0)]}/>);
 await act(async()=>{p.resolve({shot:{...shot('a',0).shot!,prompt:'Do not generate after leaving'},boundEntities:[hero]});});
 expect(api.readinessGet).not.toHaveBeenCalled();expect(api.generate).not.toHaveBeenCalled();expect(callbacks.onChanged).not.toHaveBeenCalled();
});
it('keyboard activation saves before generating; merely focusing the button does not generate',async()=>{
 const user=userEvent.setup();render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 await user.click(box);fireEvent.change(box,{target:{value:'Keyboard submitted action'}});
 act(()=>screen.getByRole('button',{name:'生成视频'}).focus());expect(api.generate).not.toHaveBeenCalled();
 await user.keyboard('{Enter}');await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));expect(api.saveShotRow).toHaveBeenCalledTimes(1);
});
it('leaving the focused Generate button without activating still autosaves, without generating',async()=>{
 const user=userEvent.setup();render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 await user.click(box);fireEvent.change(box,{target:{value:'Only save this draft'}});
 act(()=>screen.getByRole('button',{name:'生成视频'}).focus());await user.click(screen.getByLabelText('搜索分镜'));
 await waitFor(()=>expect(api.saveShotRow).toHaveBeenCalledTimes(1));expect(api.generate).not.toHaveBeenCalled();
});
it.each([['STARTED','已启动生成'],['QUEUED','已写入生产队列'],['ALREADY_ACCEPTED','已定位原运行，未重复提交'],['FAILED_TO_START','运行已保存，启动失败']])('shows %s and navigates to the exact acknowledged RunRef without retry',async(startOutcome,message)=>{
 const user=userEvent.setup(),callbacks=props();const ref={source:'queue-batch',id:'exact-acknowledged-batch'};
 api.generate.mockResolvedValueOnce({accepted:true,runRef:ref,startOutcome,startIssue:startOutcome==='FAILED_TO_START'?{code:'RUNTIME_BLOCKED',message:'blocked',details:{}}:null});
 render(<StoryboardShotRows {...callbacks} items={[shot('a',0)]}/>);await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));
 await user.click(screen.getByRole('button',{name:'生成视频'}));await screen.findByText(message);await user.click(screen.getByRole('button',{name:'查看已保存运行'}));
 expect(callbacks.onQueue).toHaveBeenCalledWith(ref);expect(api.generate).toHaveBeenCalledTimes(1);
});

it.each(['duration','name','sections'])('edited %s field should save then generate on one click',async(field)=>{
 const user=userEvent.setup(),p=deferred<unknown>();
 if(field==='sections')api.setPromptSections.mockReturnValueOnce(p.promise);else if(field==='duration')api.saveShotRow.mockReturnValueOnce(p.promise);else api.updateShot.mockReturnValueOnce(p.promise);
 render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 const button=screen.getByRole('button',{name:'生成视频'});
 await waitFor(()=>expect(button.hasAttribute('disabled')).toBe(false));
 await waitFor(()=>expect(api.promptSections).toHaveBeenCalled());
 await act(async()=>{});
 const box=screen.getByLabelText(field==='duration'?'镜头1 时长（秒）':field==='name'?'镜头1 名称':'镜头1 风格');
 await user.click(box);fireEvent.change(box,{target:{value:field==='duration'?'7':field==='name'?'Renamed shot':'Noir style'}});
 await user.click(button);
 const calls=field==='sections'?api.setPromptSections:field==='duration'?api.saveShotRow:api.updateShot;
 expect(calls).toHaveBeenCalledTimes(1);
 const disabledDuringSave=button.hasAttribute('disabled');
 await act(async()=>{p.resolve(field==='sections'?{style:'Noir style',summary:null,soundscape:null,music:null}:field==='duration'?{shot:{...shot('a',0).shot!,values:{duration_seconds:{type:'integer',value:7}}},boundEntities:[hero]}:{});});
 await waitFor(()=>expect(button.hasAttribute('disabled')).toBe(false));
 console.log(JSON.stringify({field,disabledDuringSave,saves:calls.mock.calls.length,generates:api.generate.mock.calls.length}));
 expect(api.generate,'valid first click should survive saving this field').toHaveBeenCalledTimes(1);
});
it('failed prompt save should survive hiding and restoring a row with search',async()=>{
 const user=userEvent.setup();api.saveShotRow.mockRejectedValueOnce({code:'SAVE_ERROR',message:'fixture save failure'});
 render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 await user.click(box);fireEvent.change(box,{target:{value:'An unsaved draft I need to keep'}});
 const search=screen.getByLabelText('搜索分镜');await user.click(search);await screen.findByRole('alert');
 expect((box as HTMLTextAreaElement).value).toBe('An unsaved draft I need to keep');
 fireEvent.change(search,{target:{value:'nothing-matches'}});expect(screen.queryByLabelText('镜头1 提示词')).toBeNull();
 fireEvent.change(search,{target:{value:''}});const restored=screen.getByLabelText('镜头1 提示词');
 console.log(JSON.stringify({afterRestore:(restored as HTMLTextAreaElement).value,saves:api.saveShotRow.mock.calls.length,generates:api.generate.mock.calls.length}));
 expect((restored as HTMLTextAreaElement).value).toBe('An unsaved draft I need to keep');
});
it('CONTROL prompt edit still saves and generates on one click',async()=>{
 const user=userEvent.setup();render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 await user.click(box);fireEvent.change(box,{target:{value:'A fresh prompt'}});await user.click(screen.getByRole('button',{name:'生成视频'}));
 await waitFor(()=>expect(api.generate).toHaveBeenCalledTimes(1));expect(api.saveShotRow).toHaveBeenCalledTimes(1);
});
it.each(['duration','name','sections'])('failed %s save cannot generate or preflight',async(field)=>{
 const user=userEvent.setup();const failure={code:'SAVE_ERROR',message:'field save rejected'};
 if(field==='duration')api.saveShotRow.mockRejectedValueOnce(failure);else if(field==='name')api.updateShot.mockRejectedValueOnce(failure);else api.setPromptSections.mockRejectedValueOnce(failure);
 render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);
 await waitFor(()=>expect(screen.getByRole('button',{name:'生成视频'}).hasAttribute('disabled')).toBe(false));await act(async()=>{});
 const box=screen.getByLabelText(field==='duration'?'镜头1 时长（秒）':field==='name'?'镜头1 名称':'镜头1 风格');
 await user.click(box);fireEvent.change(box,{target:{value:field==='duration'?'7':'new value'}});await user.click(screen.getByRole('button',{name:'生成视频'}));await screen.findByRole('alert');
 expect(api.generate).not.toHaveBeenCalled();expect(api.readinessGet).not.toHaveBeenCalled();
});
it('restores row search/filter on return, keeps project scopes separate and finds an unloaded shot without rich reads',async()=>{
 const callbacks=props();const index=[...Array.from({length:60},(_,i)=>({id:`loaded-${i}`,name:`已有${i}`,ordinal:i})),{id:'late-shot',name:'第61镜头',ordinal:60}];
 const view=render(<StoryboardShotRows {...callbacks} items={[shot('a',0)]} shotIndex={index}/>);
 fireEvent.change(screen.getByLabelText('搜索分镜'),{target:{value:'第61'}});fireEvent.click(screen.getByRole('button',{name:'打开第61镜头'}));expect(callbacks.onOpen).toHaveBeenCalledWith('late-shot');
 expect(api.generate).not.toHaveBeenCalled();view.unmount();
 const next=render(<StoryboardShotRows {...callbacks} items={[shot('a',0)]} shotIndex={index}/>);expect((screen.getByLabelText('搜索分镜') as HTMLInputElement).value).toBe('第61');
 next.rerender(<StoryboardShotRows {...callbacks} projectId='other-project' items={[]}/>);expect((screen.getByLabelText('搜索分镜') as HTMLInputElement).value).toBe('');
});
it('failed prompt draft survives remount but never enters a different Recipe',async()=>{
 const user=userEvent.setup();api.saveShotRow.mockRejectedValueOnce({code:'SAVE_ERROR',message:'blocked'});
 const view=render(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);const box=screen.getByLabelText('镜头1 提示词');await waitFor(()=>expect(box.hasAttribute('disabled')).toBe(false));
 await user.click(box);fireEvent.change(box,{target:{value:'Keep only for this recipe'}});await user.click(screen.getByLabelText('搜索分镜'));await screen.findByRole('alert');view.unmount();
 const next=render(<StoryboardShotRows {...props()} items={[shot('a',0,'other-recipe')]}/>);expect((screen.getByLabelText('镜头1 提示词') as HTMLTextAreaElement).value).not.toBe('Keep only for this recipe');
 next.rerender(<StoryboardShotRows {...props()} items={[shot('a',0)]}/>);await waitFor(()=>expect((screen.getByLabelText('镜头1 提示词') as HTMLTextAreaElement).value).toBe('Keep only for this recipe'));
});
