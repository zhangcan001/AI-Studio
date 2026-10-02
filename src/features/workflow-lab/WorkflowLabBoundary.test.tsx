// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
// @ts-expect-error Node helpers are test-only.
import { readFileSync, readdirSync } from "node:fs";
import { LabBenchmarkPane } from "./LabBenchmarkPane";
import { LabDiagnosticsPane } from "./LabDiagnosticsPane";
import { labCreationSelection } from "../../services/workflowLabClient";
import type { RecipeViewModel } from "../../types/generation";
const api = vi.hoisted(() => ({ list: vi.fn(async () => []), presets: vi.fn(async () => []), create: vi.fn(), queue: vi.fn() }));
vi.mock("../../services/workflowLabClient", async () => ({ ...await vi.importActual<typeof import("../../services/workflowLabClient")>("../../services/workflowLabClient"), listWorkflowBenchmarks: api.list, listPresets: api.presets, createWorkflowBenchmark: api.create, queueWorkflowBenchmark: api.queue, repairJobsStatus: vi.fn(async () => []) }));
afterEach(cleanup);
const read = (path: string): string => readFileSync(path, "utf8");
it("phase6_target13 Benchmark remains reachable as Advanced validation without implicit queue or normal draft writes", async () => {
  const catalog: RecipeViewModel[] = [{ workflowId: "wfl_A", workflowVersionId: "wfv_A", recipeId: "recipe_A", name: "技术样本", category: "image", mode: "T2I", fields: [], outputTypes: ["image"] }];
  render(<LabBenchmarkPane projectId="p" catalog={catalog} />);
  await waitFor(() => expect(api.list).toHaveBeenCalledWith("p", 20));
  fireEvent.change(screen.getByLabelText("基准配方"), { target: { value: JSON.stringify(["wfv_A", "recipe_A"]) } });
  expect(screen.getByRole("heading", { name: "高级验证 · Benchmark" })).toBeTruthy();
  expect(document.body.textContent).toContain("wfv_A"); expect(document.body.textContent).toContain("recipe_A");
  expect(api.create).not.toHaveBeenCalled(); expect(api.queue).not.toHaveBeenCalled();
  render(<LabDiagnosticsPane items={[{ workflowVersionId: "wfv_A", recipeId: "recipe_A", packageName: "exact-package", packageSourcePath: "C:/isolated/packages/exact-package", workflowSha256: "workflow-hash", recipeSha256: "recipe-hash", capability: "NOT_CHECKED", readiness: "NOT_READY", diagnostics: [{ code: "EXACT_DIAGNOSTIC", message: "只读诊断" }] }]} />);
  expect(screen.getByText("C:/isolated/packages/exact-package")).toBeTruthy();
  expect(screen.getByText("EXACT_DIAGNOSTIC")).toBeTruthy();
  expect(api.create).not.toHaveBeenCalled(); expect(api.queue).not.toHaveBeenCalled();
});
it("phase6_target14 normal surfaces have no technical Lab transport dependency and normal settings suppress legacy binding controls", () => {
  for (const file of ["src/features/create/CreatePage.tsx", "src/features/create/CreateController.ts", "src/features/runs/RunsPage.tsx", "src/features/library/LibraryPage.tsx", "src/features/generators/GeneratorSettingsPage.tsx", "src/app/v3/ProjectOverviewPage.tsx"]) {
    expect(read(file)).not.toMatch(/from ["'][^"']*(?:workflowClient|workflowLabClient|tauriClient|WorkflowWorkspace)["']/);
  }
  const host = read("src/app/App.tsx");
  expect(host).toContain('showWorkflowSettings={shellMode !== "v3"}');
  expect(host).toContain('showWorkflowRepairStatus={shellMode !== "v3"}');
  expect(read("src/features/generators/GeneratorSettingsPage.tsx")).not.toMatch(/\.workflowVersionId|\.recipeId|\.targetNode/);
});
it("phase6_target15 Advanced seam whitelists existing clients only and exact selection transport is canonical without new authority", () => {
  for (const file of readdirSync("src/features/workflow-lab").filter((f: string) => /\.tsx?$/.test(f) && !f.includes(".test."))) {
    expect(read(`src/features/workflow-lab/${file}`)).not.toMatch(/from ["'][^"']*(?:tauriClient|workflowClient)["']|\binvoke\(|\bsqlx\b|CREATE TABLE|create\(.*set/);
  }
  expect(read("src/services/workflowLabClient.ts")).not.toMatch(/export \*|\binvoke\(|fetch\(|\/prompt|INSERT INTO/);
  // The frozen Queue assertions follow the extracted execution owner, not its thin legacy wrapper.
  expect(read("src-tauri/tests/production_execution_authority_boundary.rs")).toContain("../src/features/workflows/useWorkflowLabController.ts");
  const executionOwner = read("src/features/workflows/useWorkflowLabController.ts");
  expect(executionOwner).toContain("submitGeneration({");
  expect(executionOwner).toContain("startProductionQueue(");
  expect(executionOwner).not.toMatch(/createGeneration\(\{|generateShot\(\{/);
  const ref = labCreationSelection("wfv:旧版本/🖼", "recipe:准确配方");
  expect(ref).toBe("generator:v1:" + Array.from(new TextEncoder().encode(JSON.stringify(["wfv:旧版本/🖼", "recipe:准确配方"])), b => b.toString(16).padStart(2, "0")).join(""));
  expect(ref).not.toContain("recipe:");
  expect(() => labCreationSelection("", "r")).toThrow();
  expect(() => labCreationSelection("x".repeat(5000), "r")).toThrow();
});
it("phase6_target16 real normal GeneratorSettings, advanced-only Lab and legacy rollback remain explicit without new migration", () => {
  const host = read("src/app/App.tsx");
  expect(host).toContain('<GeneratorSettingsPage'); expect(host).toContain('<WorkflowLabPage'); expect(host).toContain('workspace === "workflows" && shellMode !== "v3"');
  expect(read("src/features/workflows/WorkflowWorkspace.tsx")).toContain("useWorkflowLabController(props)");
  const nav = read("src/app/v3/AppShellV3.tsx").split('const pages =')[1].split('return <div')[0];
  expect(nav).not.toMatch(/workflow|advanced|generators/);
  expect(readdirSync("src-tauri/migrations").some((f: string) => f.startsWith("043"))).toBe(false);
});
