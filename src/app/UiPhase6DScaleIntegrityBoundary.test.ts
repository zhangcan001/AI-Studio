// @vitest-environment node
import { expect, it } from "vitest";
import { readFileSync } from "node:fs";
import {
  uiPhase6DParentReader,
  PHASE6D_MANIFEST,
  PHASE6D_PARENT,
  PHASE6D_INVARIANTS,
  PHASE6D_DATABASE,
} from "../../scripts/ui-phase6d-scale-integrity-successor-guard.mjs";
import { uiPhase6CParentReader } from "../../scripts/ui-phase6c-runs-queue-unify-successor-guard.mjs";

it("wires Phase6D scale correctness + mock E2E without claiming Native/GPU", () => {
  const scale = readFileSync("src-tauri/tests/support/library_scale_baseline.rs", "utf8");
  expect(scale).toContain("phase6d_library_scale_paging_stability_and_source_filter");
  expect(scale).toContain("performanceClaim");
  const e2e = readFileSync("src/features/Phase6DProductPath.e2e.test.tsx", "utf8");
  expect(e2e).toContain("phase6d mock path");
  expect(e2e).toContain("确认将生成视频仅作为视频参考");
  expect(e2e).toContain("expect(api.generate).not.toHaveBeenCalled()");
}, 90000);

it("accepts scoped Phase6D successor and preserves Phase6C proofs", () => {
  const reader = uiPhase6DParentReader(".");
  expect(reader.violations).toEqual([]);
  const proof = JSON.parse(readFileSync(PHASE6D_MANIFEST, "utf8"));
  expect(proof.parentHead).toBe(PHASE6D_PARENT);
  expect(proof.parentHead).toBe("acbc458671e4f9b44f548f4c9c97a035168a7a5e");
  expect(PHASE6D_INVARIANTS.observedTimingsNoFakePercent).toBe(true);
  expect(PHASE6D_INVARIANTS.nativeGpuExecuted).toBe(false);
  expect(PHASE6D_INVARIANTS.schemaChanged).toBe(false);
  expect(PHASE6D_DATABASE.migrationAfter).toBe(44);
  expect(uiPhase6CParentReader(".").violations).toEqual([]);
}, 90000);
