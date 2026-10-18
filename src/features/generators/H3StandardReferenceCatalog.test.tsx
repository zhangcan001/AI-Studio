// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { GeneratorSettingsPage } from "./GeneratorSettingsPage";
import type { GeneratorBindingSummary, GeneratorOption } from "../../product/types";

const api = vi.hoisted(() => ({ overview: vi.fn(), list: vi.fn(), set: vi.fn() }));
vi.mock("../../product/client", () => ({
  productClient: {
    project: { getOverview: api.overview, generatorBindingSet: api.set },
    creation: { generatorsList: api.list },
  },
}));

const standard: GeneratorOption = {
  selectionRef: "standard-reference-selection",
  name: "MiniMax H3 全能参考 · 标准20步（双图）",
  version: "1.0.0",
  mode: "ref2va",
  mediaKind: "video",
  availability: true,
  availabilityReason: null,
  recommended: false,
  fields: [
    { type: "textarea", key: "prompt", label: "提示词", required: true, default: "" },
    { type: "images", key: "reference_images", label: "参考图片", required: true, minItems: 1, maxItems: 2 },
  ],
};
const legacy: GeneratorOption = {
  ...standard,
  selectionRef: "legacy-reference-selection",
  name: "Existing H3 reference workflow",
  version: "2.2.0",
  recommended: true,
  fields: [
    ...standard.fields,
    { type: "videos", key: "reference_videos", label: "参考视频", required: false, minItems: 0, maxItems: 2 },
    { type: "audios", key: "reference_audios", label: "参考音频", required: false, minItems: 0, maxItems: 2 },
  ],
};
let bindings: GeneratorBindingSummary[];

beforeEach(() => {
  vi.clearAllMocks();
  bindings = [
    { stage: "VIDEO", mode: "DEFAULT", selectionRef: "legacy-reference-selection", bindingInstanceId: "default-instance", revision: 3 },
    { stage: "VIDEO", mode: "REF2VA_IMAGE", selectionRef: "legacy-reference-selection", bindingInstanceId: "image-instance", revision: 4 },
  ];
  api.overview.mockImplementation(async () => ({ generatorBindings: bindings }));
  api.list.mockImplementation(async (_project: string, stage: string) => stage === "video" ? [legacy, standard] : []);
  api.set.mockResolvedValue([]);
});
afterEach(cleanup);

it("offers the embedded standard dual-image H3 workflow as both a default video generator and the image-only reference mode", async () => {
  render(<GeneratorSettingsPage projectId="project-1" navigate={vi.fn()} />);

  const defaultSelect = await screen.findByLabelText("默认视频生成器") as HTMLSelectElement;
  expect([...defaultSelect.options].map(option => option.value)).toContain("standard-reference-selection");
  fireEvent.change(defaultSelect, { target: { value: "standard-reference-selection" } });
  fireEvent.click(screen.getAllByRole("button", { name: "保存设置" })[0]);
  await waitFor(() => expect(api.set).toHaveBeenCalledWith("project-1", {
    stage: "VIDEO",
    mode: "DEFAULT",
    selectionRef: "standard-reference-selection",
    expectedRevision: 3,
    expectedBindingInstanceId: "default-instance",
  }));

  const imageModeSelect = await screen.findByLabelText("参考图视频生成器") as HTMLSelectElement;
  expect([...imageModeSelect.options].map(option => option.value)).toContain("standard-reference-selection");
  fireEvent.change(imageModeSelect, { target: { value: "standard-reference-selection" } });
  fireEvent.click(imageModeSelect.closest("article")!.querySelector("button")!);
  await waitFor(() => expect(api.set).toHaveBeenLastCalledWith("project-1", {
    stage: "VIDEO",
    mode: "REF2VA_IMAGE",
    selectionRef: "standard-reference-selection",
    expectedRevision: 4,
    expectedBindingInstanceId: "image-instance",
  }));
});
