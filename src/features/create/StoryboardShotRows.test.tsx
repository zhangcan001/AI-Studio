// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { StoryboardShotRows } from "./StoryboardShotRows";
import type { StoryboardShot } from "./storyboardModel";

const api = vi.hoisted(() => ({ updateShot: vi.fn(), deleteShot: vi.fn(), readinessGet: vi.fn(), generate: vi.fn(), selectResult: vi.fn(), list: vi.fn(), listForShot: vi.fn(), bindShot: vi.fn(), generatorsList: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: {
  creation: { updateShot: api.updateShot, deleteShot: api.deleteShot, readinessGet: api.readinessGet, generate: api.generate, selectResult: api.selectResult, generatorsList: api.generatorsList, mediaUrl: (p: string, id: string) => `media://${p}/${id}` },
  storyEntities: { list: api.list, listForShot: api.listForShot, bindShot: api.bindShot },
  library: { imageGet: vi.fn(async () => [1]), thumbnailGet: vi.fn(async () => null) },
} }));
const hero = { id: "e1", projectId: "p", kind: "character", name: "阿离", description: "", coverAssetId: null, imageAssetIds: [], createdAt: "", updatedAt: "" } as const;
const lane = { ...hero, id: "e2", kind: "scene", name: "雨巷" } as const;
const shot = (id: string, ordinal: number, selectionRef: string | null = "sel"): StoryboardShot => ({ id, name: `镜头${ordinal + 1}`, ordinal, status: "TODO", candidates: [], run: null,
  shot: { summary: { id, name: `镜头${ordinal + 1}`, ordinal }, prompt: "@阿离 走进 @雨巷", selectionRef, values: { duration_seconds: { type: "integer", value: 5 } }, referenceAssetIds: [], selectedResultId: null, recentRun: null } });
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
  api.updateShot.mockResolvedValue({}); api.deleteShot.mockResolvedValue(undefined);
});
afterEach(() => { cleanup(); vi.clearAllMocks(); vi.restoreAllMocks(); });

it("renders real shot rows with structured description, inline @entity chips and search/status filter", async () => {
  render(<StoryboardShotRows {...props()} items={[shot("a", 0), { ...shot("b", 1), status: "COMPLETED", name: "收尾" }]} />);
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
  fireEvent.change(box, { target: { value: "竖屏 6秒" } }); fireEvent.blur(box);
  await waitFor(() => expect(api.updateShot).toHaveBeenCalledWith("p", { shotId: "b", name: "镜头1", stage: "video", prompt: "竖屏 6秒" }));
});
