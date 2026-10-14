// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { CreationContext, ProductRun } from "../../product/types";
import { StoryboardWorkbench } from "./StoryboardWorkbench";
import { deriveStoryboardStatus, runPercent } from "./storyboardModel";

const api = vi.hoisted(() => ({ get: vi.fn(), createShot: vi.fn(), runGet: vi.fn(), generate: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { creation: { get: api.get, createShot: api.createShot, generate: api.generate }, run: { get: api.runGet } } }));
afterEach(() => { cleanup(); vi.clearAllMocks(); });

const shots = [{ id: "s1", name: "镜头01", ordinal: 1 }, { id: "s2", name: "镜头02", ordinal: 2 }, { id: "s3", name: "镜头03", ordinal: 3 }];
const run = (status: ProductRun["status"], succeeded: number, total: number) => ({ ref: { source: "production-run", id: `r-${status}` }, projectId: "p", title: "run", status, phase: "", createdAt: "", updatedAt: "", progress: { total, succeeded, failed: 0, cancelled: 0 }, recoverability: { retryItemIds: [], reviewRequired: 0 }, resultsSummary: [], errorSummary: null, preferredParent: null, availableActions: [] }) as ProductRun;
function context(shotId: string | null): CreationContext {
  const shot = shots.find(s => s.id === shotId);
  return { projectId: "p", projectName: "P", stage: "video", shots, promptChoices: [], mediaInputs: [],
    selectedShot: shot ? { summary: shot, prompt: `${shot.name} prompt`, selectionRef: null, values: { duration_seconds: { type: "integer", value: 5 } }, referenceAssetIds: [], selectedResultId: shot.id === "s1" ? "a1" : null, recentRun: shot.id === "s2" ? { source: "production-run", id: "r" } : null } : null,
    candidates: shot?.id === "s1" ? [{ id: "a1", name: "a1", mediaKind: "video", selected: true }] : [] };
}
const route = { kind: "create" as const, projectId: "p", stage: "video" as const };

it("UI restore: workbench derives real shot status/stats from the product facade and never generates", async () => {
  api.get.mockImplementation(async (_p: string, shotId: string | null) => context(shotId));
  api.runGet.mockResolvedValue(run("RUNNING", 17, 25));
  const navigate = vi.fn();
  render(<StoryboardWorkbench route={route} navigate={navigate} />);
  const stats = await screen.findByLabelText("镜头统计");
  expect(within(stats).getByText("镜头总数").nextSibling?.textContent).toBe("3");
  expect(within(stats).getByText("已完成").nextSibling?.textContent).toBe("1");
  expect(within(stats).getByText("生成中").nextSibling?.textContent).toBe("1");
  expect(within(stats).getByText("待制作").nextSibling?.textContent).toBe("1");
  expect(screen.getAllByText("生成中 68%").length).toBeGreaterThan(0);
  fireEvent.click(within(screen.getByRole("article", { name: "镜头03" })).getByRole("button", { name: "生成" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "create", projectId: "p", stage: "video", shotId: "s3" });
  fireEvent.click(screen.getByRole("button", { name: "批量生成待制作镜头" }));
  expect(navigate).toHaveBeenCalledWith({ kind: "project-settings", projectId: "p", section: "advanced-shots" });
  expect(api.generate).not.toHaveBeenCalled();
});

it("UI restore: empty project shows honest empty state without demo data", async () => {
  api.get.mockResolvedValue({ ...context(null), shots: [] });
  render(<StoryboardWorkbench route={route} navigate={vi.fn()} />);
  expect(await screen.findByText("开始你的第一个镜头")).toBeTruthy();
  expect(screen.queryByRole("article")).toBeNull();
});

it("UI restore: status precedence keeps active runs ahead of selection", () => {
  expect(deriveStoryboardStatus(null, [], run("QUEUED", 0, 1))).toBe("GENERATING");
  expect(deriveStoryboardStatus(null, [{ id: "a", name: "a", mediaKind: "video", selected: false }], null)).toBe("REVIEW");
  expect(deriveStoryboardStatus(null, [], run("FAILED", 0, 1))).toBe("FAILED");
  expect(deriveStoryboardStatus(null, [], null)).toBe("TODO");
  expect(runPercent(run("RUNNING", 17, 25))).toBe(68);
  expect(runPercent(null)).toBeNull();
});
