// @vitest-environment node
import { expect, it } from "vitest";
import { readFileSync } from "node:fs";
import {
  uiPhase7AParentReader,
  PHASE7A_MANIFEST,
  PHASE7A_PARENT,
  PHASE7A_INVARIANTS,
  PHASE7A_DATABASE,
} from "../../scripts/ui-phase7a-source-ready-successor-guard.mjs";
import { uiPhase6DParentReader } from "../../scripts/ui-phase6d-scale-integrity-successor-guard.mjs";

it("wires Phase7A source gate repairs without Native/GPU claims", () => {
  const pkg = readFileSync("package.json", "utf8");
  expect(pkg).toContain("test:boundary");
  const shim = readFileSync("src/types/mjs-shims.d.ts", "utf8");
  expect(shim).toContain('declare module "*.mjs"');
  expect(shim).toContain('declare module "node:fs"');
  const retirement = readFileSync("src/app/Phase7Retirement.test.tsx", "utf8");
  expect(retirement).toContain("120000");
}, 90000);

it("accepts scoped Phase7A successor and preserves Phase6D proofs", () => {
  const reader = uiPhase7AParentReader(".");
  expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE7A_MANIFEST, "utf8"));
  expect(proof.parentHead).toBe(PHASE7A_PARENT);
  expect(proof.parentHead).toBe("0d355a166d136e2148088f799f5a94b4c914f41e");
  expect(PHASE7A_INVARIANTS.sourceGatesRepaired).toBe(true);
  expect(PHASE7A_INVARIANTS.nativeGpuExecuted).toBe(false);
  expect(PHASE7A_INVARIANTS.schemaChanged).toBe(false);
  expect(PHASE7A_DATABASE.migrationAfter).toBe(44);
  expect(uiPhase6DParentReader(".").violations).toEqual([]);
}, 120000);

it("template/history/runs reuse stay on video create, not studio/image", () => {
  const app = readFileSync("src/app/App.tsx", "utf8");
  expect(app).toMatch(/handleTemplateProjectCreated[\s\S]*?stage: "video"/);
  expect(app).toMatch(/loadHistoricalInputs[\s\S]*?stage: "video"/);
  expect(app).not.toMatch(/loadHistoricalInputs[\s\S]*?navigateToWorkspace\("studio"\)/);
  expect(app).not.toMatch(/handleTemplateProjectCreated[\s\S]*?stage: "image"/);
  expect(app).toContain("当前产品仅支持视频工作流复用");
  const runs = readFileSync("src/features/runs/RunsController.ts", "utf8");
  expect(runs).not.toContain('generatorsList(route.projectId, "image")');
  expect(runs).toContain('stage: "video"');
  const library = readFileSync("src/features/library/LibraryController.ts", "utf8");
  expect(library).toContain('returnRoute?.stage==="image"');
  expect(library).toContain('stage: "video"');
}, 30000);

it("Create generate and Queue start invalidate Runs without Library thrash", () => {
  const create = readFileSync("src/features/create/CreateController.ts", "utf8");
  expect(create).toContain('invalidateRuns(route.projectId, { library: false })');
  const queue = readFileSync("src/features/studio/ProductionQueuePanel.tsx", "utf8");
  expect(queue).toContain("projectGenerationRef");
  expect(queue).toContain('invalidateRuns(projectId, { library: false })');
  const library = readFileSync("src/features/library/LibraryController.ts", "utf8");
  expect(library).toContain("options?.library===false");
  const bus = readFileSync("src/product/runInvalidation.ts", "utf8");
  expect(bus).toContain("library?: boolean");
}, 30000);


it("formal video input invalidation refreshes Create and Inspector after batch OCC bind", () => {
  const invalidation = readFileSync("src/product/videoInputInvalidation.ts", "utf8");
  expect(invalidation).toContain("invalidateVideoInputs");
  expect(invalidation).toContain("subscribeVideoInputInvalidation");
  const bind = readFileSync("src/features/shots/ShotBatchAssetBindPanel.tsx", "utf8");
  expect(bind).toContain("invalidateVideoInputs(projectId)");
  const panel = readFileSync("src/features/shots/ShotVideoInputsPanel.tsx", "utf8");
  expect(panel).toContain("subscribeVideoInputInvalidation");
  expect(panel).toMatch(/subscribeVideoInputInvalidation[\s\S]*?void load\(\)/);
  const persistent = readFileSync("src/features/create/usePersistentVideoInputs.ts", "utf8");
  expect(persistent).toContain("subscribeVideoInputInvalidation");
  // Background invalidation preserves dirty drafts/OCC ownership and coalesces
  // reads received while our own save is in flight.
  expect(persistent).toMatch(/subscribeVideoInputInvalidation[\s\S]*?void load\(false, true\)/);
  expect(persistent).toContain("selfWrite.current");
  expect(persistent).toContain("pendingRefresh.current");
}, 30000);
