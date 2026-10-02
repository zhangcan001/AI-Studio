// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { GeneratorSettingsPage } from "./GeneratorSettingsPage";
import type { GeneratorBindingSummary, GeneratorOption } from "../../product/types";
const api = vi.hoisted(() => ({ overview: vi.fn(), list: vi.fn(), set: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { project: { getOverview: api.overview, generatorBindingSet: api.set }, creation: { generatorsList: api.list } } }));
const options: GeneratorOption[] = ["one", "two"].map(selectionRef => ({ selectionRef, name: "wfl_krea2_fixture.json", version: "2", mode: "T2I", mediaKind: "image", availability: true, availabilityReason: null, recommended: true, fields: [{ type: "textarea", key: "prompt", label: "Prompt", required: true, default: "" }] }));
let bindings: GeneratorBindingSummary[];
beforeEach(() => { vi.clearAllMocks(); bindings = [{ stage: "IMAGE", mode: "DEFAULT", selectionRef: "one", bindingInstanceId: "instance-A", revision: 3 }]; api.overview.mockImplementation(async () => ({ generatorBindings: bindings })); api.list.mockImplementation(async (_project, stage) => stage === "image" ? options : []); api.set.mockResolvedValue([]); });
afterEach(cleanup);
it("phase6_target1 friendly normal projection hides raw identities and requirements use semantic labels", async () => {
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  await screen.findByText("输入要求：提示词");
  expect(document.body.textContent).toContain("Krea 图片");
  expect(document.body.textContent).not.toMatch(/wfl_|instance-A|recipe_|workflowVersionId|nodeId|sha256/);
  expect(api.set).not.toHaveBeenCalled();
});
it("phase6_target2 default binding save uses exact opaque locator and both OCC tokens; Advanced has canonical return", async () => {
  api.list.mockImplementation(async (_project, stage) => stage === "image" ? options : [{ ...options[0], selectionRef: "video", mediaKind: "video", mode: "fl2va_text_to_video" }]);
  const navigate = vi.fn(); render(<GeneratorSettingsPage projectId="p" navigate={navigate} />);
  const select = await screen.findByLabelText("默认图片生成器");
  fireEvent.change(select, { target: { value: "two" } });
  fireEvent.click(screen.getAllByRole("button", { name: "保存设置" })[0]);
  await waitFor(() => expect(api.set).toHaveBeenCalledWith("p", { stage: "IMAGE", mode: "DEFAULT", selectionRef: "two", expectedRevision: 3, expectedBindingInstanceId: "instance-A" }));
  await waitFor(() => expect(screen.getByLabelText("文生视频生成器")).toBeTruthy());
  const mode = screen.getByLabelText("文生视频生成器");
  fireEvent.change(mode, { target: { value: "video" } });
  await waitFor(() => expect(mode.closest("article")?.querySelector("button")?.disabled).toBe(false));
  fireEvent.click(mode.closest("article")!.querySelector("button")!);
  await waitFor(() => expect(api.set).toHaveBeenLastCalledWith("p", { stage: "VIDEO", mode: "FL2VA_TEXT_TO_VIDEO", selectionRef: "video", expectedRevision: null, expectedBindingInstanceId: null }));
  fireEvent.click(screen.getByRole("button", { name: "打开高级工作流 Lab" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "system-settings", section: "advanced-workflows", returnTo: { kind: "project-settings", projectId: "p", section: "generators" } });
});
it("phase6_target3 stale and ABA conflicts refresh read projection without retry or overwrite", async () => {
  render(<GeneratorSettingsPage projectId="p" navigate={vi.fn()} />);
  const select = await screen.findByLabelText("默认图片生成器");
  for (const [instance, revision] of [["instance-A", 4], ["instance-B", 1]] as const) {
    api.set.mockRejectedValueOnce({ code: "GENERATOR_BINDING_CONFLICT", message: "technical recipe_unsafe" });
    fireEvent.change(select, { target: { value: "two" } });
    bindings = [{ stage: "IMAGE", mode: "DEFAULT", selectionRef: "one", bindingInstanceId: instance, revision }];
    fireEvent.click(screen.getAllByRole("button", { name: "保存设置" })[0]);
    await screen.findByRole("alert"); await waitFor(() => expect((select as HTMLSelectElement).value).toBe("one"));
    expect(document.body.textContent).not.toContain("recipe_unsafe");
    fireEvent.click(screen.getByRole("button", { name: "刷新设置" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
  }
  expect(api.set).toHaveBeenCalledTimes(2);
  expect(api.set.mock.calls[1][1]).toMatchObject({ expectedRevision: 4, expectedBindingInstanceId: "instance-A" });
});
