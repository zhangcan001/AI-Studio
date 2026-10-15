// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { StoryEntitiesSection } from "./StoryEntitiesSection";

const api = vi.hoisted(() => ({ list: vi.fn(), create: vi.fn(), update: vi.fn(), delete: vi.fn(), imageGet: vi.fn(), libList: vi.fn(), tagsList: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { storyEntities: { list: api.list, create: api.create, update: api.update, delete: api.delete }, library: { imageGet: api.imageGet, list: api.libList, tagsList: api.tagsList } } }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });

const entity = { id: "e1", projectId: "p", kind: "character", name: "苏蔓", description: "", coverAssetId: "a1", imageAssetIds: ["a1"], createdAt: "", updatedAt: "" };

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