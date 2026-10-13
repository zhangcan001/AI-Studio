// Phase 7B Native N4 TSV preview repair successor; inherited historical proofs remain immutable.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { uiPhase7BTsvReceiptParentReader } from "./ui-phase7b-tsv-receipt-eligibility-successor-guard.mjs";
import { phase7BHydrationScopeSince } from "./ui-phase7b-workspace-hydration-successor-guard.mjs";

export const PHASE7B_TSV_BASE = "192d2358549f57400bbb06ca04c1e6a106969739";
export const PHASE7B_TSV_PROOF = "docs/architecture/ui-phase7b-tsv-preview-successor.json";
export const PHASE7B_TSV_CHANGED = [
  "scripts/ui-phase7b-ref2va-empty-readiness-successor-guard.mjs",
  "src/features/shots/ShotBatchAssetBindPanel.tsx",
  "src/features/shots/ShotBatchAssetBindPanel.test.tsx",
].sort();
export const PHASE7B_TSV_ADDED = [
  "scripts/ui-phase7b-tsv-preview-successor-guard.mjs",
  "src/app/UiPhase7BTsvPreviewBoundary.test.ts",
].sort();

const canonical = value => value.replaceAll("\r\n", "\n");
const sha = value => createHash("sha256").update(canonical(value)).digest("hex");
function git(root, args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
}
function ancestor(root, path) {
  return canonical(git(root, ["show", PHASE7B_TSV_BASE + ":" + path]));
}

export function uiPhase7BTsvParentReader(root, override = undefined) {
  const workspace = resolve(root);
  const tip = uiPhase7BTsvReceiptParentReader(root, override);
  const violations = [...tip.violations];
  const fail = id => violations.push("ui-phase7b-tsv-preview-" + id);
  const paths = [...PHASE7B_TSV_CHANGED, ...PHASE7B_TSV_ADDED].sort();
  const current = path => canonical(override && Object.hasOwn(override, path)
    ? override[path] : tip.read(path));
  let manifest;
  try { manifest = JSON.parse(readFileSync(join(workspace, PHASE7B_TSV_PROOF), "utf8")); }
  catch { fail("missing-proof"); }
  if (!manifest || manifest.schemaVersion !== 1
      || manifest.checkpoint !== "UI_PHASE7B_NATIVE_TSV_PREVIEW_FIX"
      || manifest.parentHead !== PHASE7B_TSV_BASE
      || JSON.stringify(Object.keys(manifest.files ?? {}).sort()) !== JSON.stringify(paths)) fail("proof-contract");
  try {
    if (git(workspace, ["rev-parse", PHASE7B_TSV_BASE]).trim() !== PHASE7B_TSV_BASE) fail("ancestor");
    const expected = [...new Set([...paths, PHASE7B_TSV_PROOF, ...tip.changedPaths, ...tip.addedPaths])].sort();
    const relevant = phase7BHydrationScopeSince(workspace, PHASE7B_TSV_BASE);
    if (JSON.stringify(relevant) !== JSON.stringify(expected)) fail("unapproved-source");
    for (const path of PHASE7B_TSV_CHANGED) {
      if (!existsSync(join(workspace, path)) || manifest.files[path]?.beforeHash !== sha(ancestor(workspace, path))
          || manifest.files[path]?.afterHash !== sha(current(path))) fail("changed:" + path);
    }
    for (const path of PHASE7B_TSV_ADDED) {
      if (!existsSync(join(workspace, path)) || manifest.files[path]?.beforeHash !== null
          || manifest.files[path]?.afterHash !== sha(current(path))) fail("added:" + path);
    }
    const source = current("src/features/shots/ShotBatchAssetBindPanel.tsx");
    // Frozen TSV tip still requires scoped library preview + OCC; receipt tip owns newer panel bytes.
    if (!source.includes("productClient.library.get(projectId")
        || !readFileSync(join(workspace, "src/features/shots/ShotBatchAssetBindPanel.tsx"), "utf8").includes("await validateTsvAssets(resolveBatchBindRows")
        || !readFileSync(join(workspace, "src/features/shots/ShotBatchAssetBindPanel.tsx"), "utf8").includes("await validateTsvAssets(previewLines")
        || !readFileSync(join(workspace, "src/features/shots/ShotBatchAssetBindPanel.tsx"), "utf8").includes("disabled={busy || !tsvValid}")) fail("preview-guard");
    const live = readFileSync(join(workspace, "src/features/shots/ShotBatchAssetBindPanel.tsx"), "utf8");
    if (!live.includes("inputsSave({ selection, expected: view.token, inputs })")
        || live.includes("startProductionQueue")) fail("single-queue-and-occ");
  } catch (e) { fail("validation:" + String(e).slice(0, 120)); }
  return {
    violations,
    changedPaths: [...new Set([...PHASE7B_TSV_CHANGED, ...tip.changedPaths])].sort(),
    addedPaths: [...new Set([...PHASE7B_TSV_ADDED, PHASE7B_TSV_PROOF, ...tip.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries(paths.map(path => [path, manifest?.files?.[path]?.afterHash ?? ""])), ...tip.afterHashes },
    read: path => PHASE7B_TSV_CHANGED.includes(path) ? ancestor(workspace, path) : current(path),
  };
}
