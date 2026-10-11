// @vitest-environment node
import { expect, it } from "vitest";
import { readFileSync } from "node:fs";
import {
  uiPhase6BParentReader,
  PHASE6B_MANIFEST,
  PHASE6B_PARENT,
  PHASE6B_INVARIANTS,
  PHASE6B_DATABASE,
} from "../../scripts/ui-phase6b-library-shot-bind-successor-guard.mjs";
import { uiPhase6AParentReader } from "../../scripts/ui-phase6a-library-source-successor-guard.mjs";

it("wires Library→exact Shot video input bind without Task/Run or schema change", () => {
  const hook = readFileSync("src/features/create/usePersistentVideoInputs.ts", "utf8");
  expect(hook).toContain("async function bindAsset");
  expect(hook).toContain("inputsSave");
  const controller = readFileSync("src/features/create/CreateController.ts", "utf8");
  expect(controller).toContain("videoInputs.bindAsset");
  expect(controller).toContain("libraryNeedsGeneratedVideoConfirm");
  const page = readFileSync("src/features/create/CreatePage.tsx", "utf8");
  expect(page).toContain("确认将生成视频仅作为视频参考");
  const relations = readFileSync("src-tauri/src/application/product/library_facade/relations.rs", "utf8");
  expect(relations).toContain("shot_video_input_ids");
  expect(relations).toContain("正式视频输入");
}, 90000);

it("accepts scoped Phase6B successor and preserves Phase6A proofs", () => {
  const reader = uiPhase6BParentReader(".");
  expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE6B_MANIFEST, "utf8"));
  expect(proof.parentHead).toBe(PHASE6B_PARENT);
  expect(proof.parentHead).toBe("32048fd3213597572c807fdc0a3e9cfa19c52752");
  expect(PHASE6B_INVARIANTS.libraryExactShotInputBind).toBe(true);
  expect(PHASE6B_INVARIANTS.schemaChanged).toBe(false);
  expect(PHASE6B_DATABASE.migrationAfter).toBe(44);
  expect(PHASE6B_DATABASE.backupAfter).toBe(21);
  expect(uiPhase6AParentReader(".").violations).toEqual([]);
}, 90000);
