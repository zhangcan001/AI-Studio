// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ShotView } from "../../types/shot";
import { ShotBulkConfigPanel } from "./ShotBulkConfigPanel";
import { SeriesProductionPanel } from "./SeriesProductionPanel";
import { EpisodeProductionPanel } from "./EpisodeProductionPanel";

const api = vi.hoisted(() => ({ preflight: vi.fn(), admit: vi.fn(), legacyPlan: vi.fn(), legacyPrepare: vi.fn(), presets: vi.fn(), start: vi.fn() }));
vi.mock("../../services/tauriClient", () => ({
  getProjectProductionPreflight: api.preflight, admitProjectProduction: api.admit,
  getEpisodeProductionPlan: api.legacyPlan, prepareEpisodeProduction: api.legacyPrepare,
  listBatchWorkflowPresets: api.presets, bulkSetShotStageConfig: vi.fn(), startProductionQueue: api.start,
}));
const shot = (id: string, ordinal = 0): ShotView => ({ id, projectId: "p", ordinal, name: id, promptText: "move", status: "DRAFT", imageStatus: "DRAFT", videoStatus: "DRAFT", stageConfigs: [], referenceAssets: [], generationLinks: [], createdAt: "", updatedAt: "" });
const scene = (id: string, episodeId: string, shotIds: string[], ordinal = 0) => ({ id, episodeId, shotIds, ordinal, name: id, description: "", createdAt: "", updatedAt: "" });
const tree: ProductionStructureTree = { projectId: "p", unassignedShotIds: ["legacy"], series: [{ id: "s", projectId: "p", ordinal: 0, name: "series", description: "", createdAt: "", updatedAt: "", episodes: [
  { id: "e2", seriesId: "s", ordinal: 1, name: "episode2", description: "", createdAt: "", updatedAt: "", scenes: [scene("c2", "e2", ["ref"])] },
  { id: "e1", seriesId: "s", ordinal: 0, name: "episode1", description: "", createdAt: "", updatedAt: "", scenes: [scene("c1", "e1", ["fl", "i2v", "t2v"])] },
] }] };
const shots = [shot("t2v", 0), shot("ref", 0), shot("i2v", 1), shot("fl", 2), shot("legacy", 0)];
const rows = shots.map(s => ({ shotId: s.id, ordinal: s.ordinal, name: s.name, status: "READY", score: 100, warningCount: 0, incompleteCount: 0, blockerCount: 0, contextHash: s.id, workflowVersionId: `v-${s.id}`, recipeId: s.id, alreadyPrepared: false, blockers: [] }));
const plan = { projectId: "p", stage: "video", total: 5, readyCount: 5, blockedCount: 0, incompleteCount: 0, warningCount: 0, preparedCount: 0, items: rows };
beforeEach(() => { vi.clearAllMocks(); api.preflight.mockResolvedValue(plan); api.presets.mockResolvedValue([]); api.admit.mockResolvedValue({ projectId: "p", stage: "video", createdCount: 4, alreadyPreparedCount: 0, skippedIncomplete: 0, skippedBlocked: 0, createdBatchIds: ["mixed"] }); });
afterEach(cleanup);

it.each([
  ["project", undefined, ["fl", "i2v", "t2v", "ref", "legacy"]],
  ["series", "s", ["fl", "i2v", "t2v", "ref"]],
  ["episode", "e1", ["fl", "i2v", "t2v"]],
  ["scene", "c2", ["ref"]],
  ["unassigned", undefined, ["legacy"]],
] as const)("prepares %s scope in hierarchy/scene order, not global Shot ordinal", async (kind, id, expected) => {
  const user = userEvent.setup();
  render(<ShotBulkConfigPanel projectId="p" shots={shots} tree={tree} scope={{ kind, id }} />);
  await waitFor(() => expect(api.preflight).toHaveBeenCalled());
  await user.click(screen.getByRole("button", { name: "选择就绪项" }));
  await user.click(screen.getByRole("button", { name: "准备视频生产" }));
  await waitFor(() => expect(api.admit).toHaveBeenCalledTimes(1));
  expect(api.admit).toHaveBeenCalledWith({ projectId: "p", stage: "video", shotIds: [...expected], allowPartial: false, scope: { kind, id } });
  expect(api.start).not.toHaveBeenCalled();
});

it.each([SeriesProductionPanel, EpisodeProductionPanel])("mounted hierarchy panel uses formal preflight/admit, not legacy prepare", async Panel => {
  const user = userEvent.setup(), legacyPlan = vi.fn(), legacyPrepare = vi.fn();
  render(<Panel projectId="p" tree={tree} shots={shots} onPlan={legacyPlan} onPrepare={legacyPrepare} />);
  await screen.findByText("v-fl / fl");
  await user.click(screen.getByRole("button", { name: "选择就绪项" }));
  await user.click(screen.getByRole("button", { name: "准备视频生产" }));
  await waitFor(() => expect(api.admit).toHaveBeenCalledTimes(1));
  expect(legacyPlan).not.toHaveBeenCalled(); expect(legacyPrepare).not.toHaveBeenCalled();
  expect(api.legacyPlan).not.toHaveBeenCalled(); expect(api.legacyPrepare).not.toHaveBeenCalled(); expect(api.start).not.toHaveBeenCalled();
});

it("does not reuse late same-project admission after switching hierarchy scope", async () => {
  let resolve!: (v: unknown) => void;
  api.admit.mockReturnValue(new Promise(r => { resolve = r; }));
  const user = userEvent.setup(), notice = vi.fn();
  const { rerender } = render(<ShotBulkConfigPanel projectId="p" shots={shots} tree={tree} scope={{ kind: "episode", id: "e1" }} onNotice={notice} />);
  await screen.findByText("v-fl / fl");
  await user.click(screen.getByRole("button", { name: "全选" })); await user.click(screen.getByRole("button", { name: "准备视频生产" }));
  await waitFor(() => expect(api.admit).toHaveBeenCalledTimes(1)); notice.mockClear();
  rerender(<ShotBulkConfigPanel projectId="p" shots={shots} tree={tree} scope={{ kind: "episode", id: "e2" }} onNotice={notice} />);
  await screen.findByText("v-ref / ref");
  await act(async () => resolve({ projectId: "p", stage: "video", createdCount: 3, createdBatchIds: ["OLD"] }));
  expect(screen.queryByText(/已准备完成/)).toBeNull(); expect(notice).not.toHaveBeenCalled();
});

it("rejects foreign hierarchy instead of widening selection to all project Shots", async () => {
  render(<ShotBulkConfigPanel projectId="p" shots={shots} tree={{ ...tree, projectId: "foreign" }} scope={{ kind: "series", id: "s" }} />);
  await screen.findByRole("alert"); expect(api.preflight).not.toHaveBeenCalled(); expect(api.admit).not.toHaveBeenCalled();
});

it("releases a stale admission lock when a same-scope hierarchy refresh changes Shot membership", async () => {
  let resolve!: (v: unknown) => void;
  api.admit.mockReturnValueOnce(new Promise(r => { resolve = r; }));
  const user = userEvent.setup(), notice = vi.fn();
  const props = { projectId: "p", shots, scope: { kind: "episode" as const, id: "e1" }, onNotice: notice };
  const { rerender } = render(<ShotBulkConfigPanel {...props} tree={tree} />);
  await screen.findByText("v-fl / fl");
  await user.click(screen.getByRole("button", { name: "全选" }));
  await user.click(screen.getByRole("button", { name: "准备视频生产" }));
  await waitFor(() => expect(api.admit).toHaveBeenCalledTimes(1));
  const changed = structuredClone(tree);
  changed.series[0].episodes[1].scenes[0].shotIds = ["i2v", "t2v"];
  notice.mockClear(); rerender(<ShotBulkConfigPanel {...props} tree={changed} />);
  await act(async () => resolve({ projectId: "p", stage: "video", createdCount: 3, createdBatchIds: ["OLD"] }));
  expect(screen.queryByText(/已准备完成/)).toBeNull(); expect(notice).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "选择就绪项" }));
  await user.click(screen.getByRole("button", { name: "准备视频生产" }));
  await waitFor(() => expect(api.admit).toHaveBeenCalledTimes(2));
  expect(api.admit.mock.calls[1][0].shotIds).toEqual(["i2v", "t2v"]);
});

it("requires explicit partial mode, displays actual reused/skipped counts and opens reused batch without starting it", async () => {
  api.preflight.mockResolvedValue({ ...plan, items: rows.map(r => ({ ...r, status: r.shotId === "i2v" ? "INCOMPLETE" : "READY", alreadyPrepared: r.shotId === "fl" })) });
  api.admit.mockResolvedValue({ projectId: "p", stage: "video", createdCount: 0, alreadyPreparedCount: 1, skippedIncomplete: 1, skippedBlocked: 0, createdBatchIds: [], matchingPreparedBatchIds: ["reused"] });
  const user = userEvent.setup(), open = vi.fn();
  render(<ShotBulkConfigPanel projectId="p" shots={shots} tree={tree} scope={{ kind: "episode", id: "e1" }} onOpenProductionQueue={open} />);
  await screen.findByText("v-fl / fl"); await user.click(screen.getByRole("button", { name: "全选" }));
  await user.click(screen.getByRole("button", { name: "准备视频生产" })); await screen.findByRole("alert"); expect(api.admit).not.toHaveBeenCalled();
  await user.click(screen.getByRole("checkbox", { name: /允许部分准备/ })); await user.click(screen.getByRole("button", { name: "准备视频生产" }));
  await waitFor(() => expect(api.admit).toHaveBeenCalledTimes(1)); expect(api.admit.mock.calls[0][0].allowPartial).toBe(true);
  await user.click(await screen.findByRole("button", { name: "打开生产队列" })); expect(open).toHaveBeenCalledWith("reused"); expect(api.start).not.toHaveBeenCalled();
});
