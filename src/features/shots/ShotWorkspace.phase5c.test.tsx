// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { ShotWorkspace } from "./ShotWorkspace";
import type { ShotView } from "../../types/shot";
import type { RecipeViewModel } from "../../types/generation";
import type { ShotCreationWorkspaceProps } from "./ShotCreationWorkspace";
const api = vi.hoisted(() => ({ select: vi.fn(), shots: vi.fn() }));
vi.mock("../../services/tauriClient", async original => ({ ...await original<typeof import("../../services/tauriClient")>(),
  listShots: api.shots, selectShotResult: api.select,
  getProjectWorkflowConfig: vi.fn(async () => ({ projectId: "p", videoModeOverrides: [] })),
  listRecentAssets: vi.fn(async () => ["x", "y"].map(id => ({ id, assetType: "video", category: "source_video", name: id, mimeType: "video/mp4", fileSize: 1, createdAt: "", isFavorite: false, tags: [] }))),
  listPromptLibrary: vi.fn(async () => ({ items: [], total: 0 })), listReferenceAnchors: vi.fn(async () => []), listBatchWorkflowPresets: vi.fn(async () => []),
  listProductionStructure: vi.fn(async () => ({ projectId: "p", series: [], unassignedShotIds: ["a", "b"] })),
  getProductionBatchRunbook: vi.fn(async () => ({ projectId: "p", rows: [] })), listProductionQueues: vi.fn(async () => []), getProductionAdmissionStatus: vi.fn(async () => ({ busy: false })),
}));
vi.mock("./ProjectStructureTree", () => ({ ProjectStructureTree: ({ onSelectSelection }: { onSelectSelection: (v: unknown) => void }) => <button onClick={() => onSelectSelection({ type: "shot", shotId: "b" })}>切换镜头B</button> }));
vi.mock("./ShotCreationWorkspace", () => ({ ShotCreationWorkspace: (p: ShotCreationWorkspaceProps) => <div>
  {p.error && <p role="alert">{p.error}</p>}
  <span data-testid="owner">{p.shot?.id}</span><span data-testid="recipe">{p.currentRecipe?.recipeId}</span>
  <button onClick={() => void p.onCandidateConfirm?.("x", false)}>测试显式选用</button>
  <button onClick={() => p.onRecipeChange?.("rb", "vb")}>切换RecipeB</button>
</div> }));
const shot = (id: string, ordinal: number): ShotView => ({ id, projectId: "p", ordinal, name: id, promptText: "move", status: "DRAFT", imageStatus: "DRAFT", videoStatus: "DRAFT", createdAt: "", updatedAt: "", stageConfigs: [{ stage: "video", workflowVersionId: "va", recipeId: "ra", scalarValues: {}, updatedAt: "" }], referenceAssets: [], generationLinks: [] });
const a = shot("a", 0), b = shot("b", 1);
const catalog: RecipeViewModel[] = ["a", "b"].map(x => ({ workflowId: "w", workflowVersionId: `v${x}`, recipeId: `r${x}`, name: `recipe${x}`, category: "video", mode: "text_to_video", outputTypes: ["video"], fields: [] }));
beforeEach(() => { vi.clearAllMocks(); api.shots.mockResolvedValue([a, b]); }); afterEach(cleanup);
it("does not let a late selection receipt navigate back to its old Shot", async () => {
  let finish!: (v: ShotView) => void; api.select.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  render(<ShotWorkspace projectId="p" catalog={catalog} initialSelectedShotId="a" />);
  await waitFor(() => expect(screen.getByTestId("owner").textContent).toBe("a"));
  await userEvent.setup().click(screen.getByRole("button", { name: "测试显式选用" }));
  await userEvent.setup().click(screen.getByRole("button", { name: "切换镜头B" }));
  await waitFor(() => expect(screen.getByTestId("owner").textContent).toBe("b"));
  await act(async () => finish({ ...a, selectedVideoAssetId: "x" }));
  expect(screen.getByTestId("owner").textContent).toBe("b");
});
it("keeps the current Recipe draft after an old selection receipt settles", async () => {
  let finish!: (v: ShotView) => void; api.select.mockReturnValueOnce(new Promise(resolve => { finish = resolve; }));
  render(<ShotWorkspace projectId="p" catalog={catalog} initialSelectedShotId="a" />);
  await waitFor(() => expect(screen.getByTestId("recipe").textContent).toBe("ra"));
  await userEvent.setup().click(screen.getByRole("button", { name: "测试显式选用" }));
  await userEvent.setup().click(screen.getByRole("button", { name: "切换RecipeB" }));
  await waitFor(() => expect(screen.getByTestId("recipe").textContent).toBe("rb"));
  await act(async () => finish({ ...a, selectedVideoAssetId: "x" }));
  expect(screen.getByTestId("recipe").textContent).toBe("rb");
});
it("rejects a selection acknowledgement with a foreign project or Shot", async () => {
  api.select.mockResolvedValue({ ...a, id: "foreign", projectId: "other" });
  render(<ShotWorkspace projectId="p" catalog={catalog} initialSelectedShotId="a" />);
  await waitFor(() => expect(screen.getByTestId("owner").textContent).toBe("a"));
  await userEvent.setup().click(screen.getByRole("button", { name: "测试显式选用" }));
  await screen.findByRole("alert"); expect(screen.getByTestId("owner").textContent).toBe("a");
});
