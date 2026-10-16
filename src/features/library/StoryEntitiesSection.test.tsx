// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { StoryEntitiesSection } from "./StoryEntitiesSection";
import type { StoryEntity } from "../../types/storyEntity";
import { STORY_ENTITIES_CHANGED } from "../../types/storyEntity";

const api = vi.hoisted(() => ({ list: vi.fn(), create: vi.fn(), update: vi.fn(), delete: vi.fn(), imageGet: vi.fn(), libList: vi.fn(), tagsList: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { storyEntities: { list: api.list, create: api.create, update: api.update, delete: api.delete }, library: { imageGet: api.imageGet, list: api.libList, tagsList: api.tagsList } } }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });

const entity: StoryEntity = { id: "e1", projectId: "p", kind: "character", name: "苏蔓", description: "", coverAssetId: "a1", imageAssetIds: ["a1"], createdAt: "", updatedAt: "" };

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

it.each(["success", "error"])("ignores late %s from the previous type filter", async outcome => {
  const old = deferred<typeof entity[]>(), current = deferred<typeof entity[]>();
  api.list.mockImplementation((_project, kind) => kind === "character" ? old.promise : kind === "scene" ? current.promise : Promise.resolve([]));
  render(<StoryEntitiesSection projectId="p" />);
  const tabs = screen.getByRole("tablist", { name: "实体类型" });
  fireEvent.click(within(tabs).getByRole("tab", { name: "角色" }));
  await waitFor(() => expect(api.list).toHaveBeenCalledWith("p", "character"));
  fireEvent.click(within(tabs).getByRole("tab", { name: "场景" }));
  await waitFor(() => expect(api.list).toHaveBeenCalledWith("p", "scene"));
  const scene: StoryEntity = { ...entity, id: "scene", kind: "scene", name: "当前场景", coverAssetId: null, imageAssetIds: [] };
  await act(async () => { current.resolve([scene]); });
  expect(screen.getByText("当前场景")).toBeTruthy();
  await act(async () => { if (outcome === "success") old.resolve([{ ...entity, coverAssetId: null, imageAssetIds: [] }]); else old.reject(new Error("stale error")); });
  expect(screen.getByText("当前场景")).toBeTruthy();
  expect(screen.queryByText("苏蔓")).toBeNull();
  expect(screen.queryByRole("alert")).toBeNull();
});

it("does not clear the current loading state when an older request finishes", async () => {
  const old = deferred<typeof entity[]>(), current = deferred<typeof entity[]>();
  api.list.mockImplementation((_project, kind) => kind === "character" ? old.promise : kind === "scene" ? current.promise : Promise.resolve([]));
  render(<StoryEntitiesSection projectId="p" />);
  const tabs = screen.getByRole("tablist", { name: "实体类型" });
  fireEvent.click(within(tabs).getByRole("tab", { name: "角色" }));
  fireEvent.click(within(tabs).getByRole("tab", { name: "场景" }));
  await waitFor(() => expect(api.list).toHaveBeenCalledWith("p", "scene"));
  await act(async () => { old.resolve([]); });
  expect(screen.getByText("加载中…")).toBeTruthy();
  expect(screen.getByRole("button", { name: "新建" }).hasAttribute("disabled")).toBe(true);
  await act(async () => { current.resolve([]); });
  expect(screen.queryByText("加载中…")).toBeNull();
});

it("keeps accepted mutation invalidation after unmount without reloading the old view", async () => {
  const pending = deferred<StoryEntity>();
  api.list.mockResolvedValue([]); api.create.mockReturnValue(pending.promise);
  const notified = vi.fn(); window.addEventListener(STORY_ENTITIES_CHANGED, notified);
  try {
    const view = render(<StoryEntitiesSection projectId="p" />);
    await waitFor(() => expect(screen.getByRole("button", { name: "新建" }).hasAttribute("disabled")).toBe(false));
    fireEvent.click(screen.getByRole("button", { name: "新建" }));
    const dialog = screen.getByRole("dialog", { name: "新建实体" });
    fireEvent.change(within(dialog).getByLabelText("实体名称"), { target: { value: "新角色" } });
    fireEvent.click(within(dialog).getByRole("button", { name: "保存" }));
    await waitFor(() => expect(api.create).toHaveBeenCalled());
    const readCount = api.list.mock.calls.length;
    view.unmount();
    await act(async () => { pending.resolve(entity); });
    expect(notified).toHaveBeenCalledTimes(1);
    expect(api.list).toHaveBeenCalledTimes(readCount);
  } finally { window.removeEventListener(STORY_ENTITIES_CHANGED, notified); }
});

it("creates a story entity through the formal command and shows it after reload", async () => {
  let saved = false;
  api.list.mockImplementation(async () => saved ? [entity] : []);
  api.create.mockImplementation(async () => { saved = true; return entity; });
  api.imageGet.mockResolvedValue([137, 80, 78, 71]);
  Object.assign(URL, { createObjectURL: () => "blob:x", revokeObjectURL: () => undefined });
  render(<StoryEntitiesSection projectId="p" />);
  await waitFor(() => expect(api.list).toHaveBeenCalled());
  fireEvent.click(await screen.findByRole("button", { name: "新建" }));
  const dialog = screen.getByRole("dialog", { name: "新建实体" });
  fireEvent.change(within(dialog).getByLabelText("实体名称"), { target: { value: "苏蔓" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.create).toHaveBeenCalledWith({ projectId: "p", kind: "character", name: "苏蔓", description: "", coverAssetId: null, imageAssetIds: [] }));
  expect(await screen.findByText("苏蔓")).toBeTruthy();
  expect(screen.queryByRole("dialog", { name: "新建实体" })).toBeNull();
});

it("edits an existing entity through update and surfaces backend errors", async () => {
  api.list.mockResolvedValue([entity]);
  Object.assign(URL, { createObjectURL: () => "blob:x", revokeObjectURL: () => undefined });
  api.update.mockRejectedValueOnce({ code: "INVALID_INPUT", message: "同类型下名称已存在" });
  api.imageGet.mockResolvedValue([137, 80, 78, 71]);
  render(<StoryEntitiesSection projectId="p" />);
  fireEvent.click(await screen.findByRole("button", { name: "编辑" }));
  const dialog = screen.getByRole("dialog", { name: "编辑实体" });
  fireEvent.click(within(dialog).getByRole("button", { name: "保存" }));
  await waitFor(() => expect(api.update).toHaveBeenCalledWith(expect.objectContaining({ projectId: "p", id: "e1", name: "苏蔓" })));
  expect((await screen.findByRole("alert")).textContent).toContain("同类型下名称已存在");
  expect(within(screen.getByRole("dialog", { name: "编辑实体" })).getByRole("button", { name: "保存" }).hasAttribute("disabled")).toBe(false);
});
