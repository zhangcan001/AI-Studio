// Phase 7B TSV receipt-eligibility repair successor; frozen TSV preview proof stays immutable.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { uiPhase7BWorkspaceHydrationParentReader, phase7BHydrationScopeSince } from "./ui-phase7b-workspace-hydration-successor-guard.mjs";

export const PHASE7B_RECEIPT_BASE = "e7362dc4a68aaeb8e95092ed1154a1e54c7caf39";
export const PHASE7B_RECEIPT_PROOF = "docs/architecture/ui-phase7b-tsv-receipt-eligibility-successor.json";
export const PHASE7B_RECEIPT_CHANGED = [
  "scripts/ui-phase7b-tsv-preview-successor-guard.mjs",
  "src/features/shots/ShotBatchAssetBindPanel.tsx",
  "src/features/shots/ShotBatchAssetBindPanel.test.tsx",
  "src-tauri/src/application/asset_query_service.rs",
  "src/types/asset.ts",
].sort();
export const PHASE7B_RECEIPT_ADDED = [
  "scripts/ui-phase7b-tsv-receipt-eligibility-successor-guard.mjs",
  "src/app/UiPhase7BTsvReceiptEligibilityBoundary.test.ts",
].sort();

const canonical = value => value.replaceAll("\r\n", "\n");
const sha = value => createHash("sha256").update(canonical(value)).digest("hex");
function git(root, args) {
  return execFileSync("git", args, { cwd: root, encoding: "utf8", maxBuffer: 16 * 1024 * 1024 });
}
function ancestor(root, path) {
  return canonical(git(root, ["show", PHASE7B_RECEIPT_BASE + ":" + path]));
}

export function uiPhase7BTsvReceiptParentReader(root, override = undefined) {
  const workspace = resolve(root);
  const tip = uiPhase7BWorkspaceHydrationParentReader(root, override);
  const violations = [...tip.violations];
  const fail = id => violations.push("ui-phase7b-tsv-receipt-" + id);
  const paths = [...PHASE7B_RECEIPT_CHANGED, ...PHASE7B_RECEIPT_ADDED].sort();
  const current = path => canonical(override && Object.hasOwn(override, path)
    ? override[path] : tip.read(path));
  let manifest;
  try { manifest = JSON.parse(current(PHASE7B_RECEIPT_PROOF)); }
  catch { fail("missing-proof"); }
  if (!manifest || manifest.schemaVersion !== 1
      || manifest.checkpoint !== "UI_PHASE7B_NATIVE_TSV_RECEIPT_ELIGIBILITY"
      || manifest.parentHead !== PHASE7B_RECEIPT_BASE
      || JSON.stringify(Object.keys(manifest.files ?? {}).sort()) !== JSON.stringify(paths)) fail("proof-contract");
  try {
    if (git(workspace, ["rev-parse", PHASE7B_RECEIPT_BASE]).trim() !== PHASE7B_RECEIPT_BASE) fail("ancestor");
    const expected = [...new Set([...paths, PHASE7B_RECEIPT_PROOF, ...tip.changedPaths, ...tip.addedPaths])].sort();
    const relevant = phase7BHydrationScopeSince(workspace, PHASE7B_RECEIPT_BASE);
    if (JSON.stringify(relevant) !== JSON.stringify(expected)) fail("unapproved-source");
    for (const path of PHASE7B_RECEIPT_CHANGED) {
      if (!existsSync(join(workspace, path)) || manifest.files[path]?.beforeHash !== sha(ancestor(workspace, path))
          || manifest.files[path]?.afterHash !== sha(current(path))) fail("changed:" + path);
    }
    for (const path of PHASE7B_RECEIPT_ADDED) {
      if (!existsSync(join(workspace, path)) || manifest.files[path]?.beforeHash !== null
          || manifest.files[path]?.afterHash !== sha(current(path))) fail("added:" + path);
    }
    const panel = current("src/features/shots/ShotBatchAssetBindPanel.tsx");
    const query = current("src-tauri/src/application/asset_query_service.rs");
    if (!panel.includes("hasExternalImportReceipt !== true")
        || !panel.includes("await validateTsvAssets(resolveBatchBindRows")
        || !panel.includes("await validateTsvAssets(previewLines")
        || !panel.includes("productClient.library.get(projectId")
        || !panel.includes("disabled={busy || !tsvValid}")) fail("preview-guard");
    if (!query.includes("has_external_import_receipt")
        || !query.includes("external_import_receipt_matches")
        || !query.includes("find_external_import(project_id, &asset_id)")) fail("receipt-gate");
    if (!panel.includes("inputsSave({ selection, expected: view.token, inputs })")
        || panel.includes("startProductionQueue")) fail("single-queue-and-occ");
  } catch (e) { fail("validation:" + String(e).slice(0, 120)); }
  return {
    violations,
    changedPaths: [...new Set([...PHASE7B_RECEIPT_CHANGED, ...tip.changedPaths])].sort(),
    addedPaths: [...new Set([...PHASE7B_RECEIPT_ADDED, PHASE7B_RECEIPT_PROOF, ...tip.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries(paths.map(path => [path, manifest?.files?.[path]?.afterHash ?? ""])), ...tip.afterHashes },
    read: path => PHASE7B_RECEIPT_CHANGED.includes(path) ? ancestor(workspace, path) : current(path),
  };
}
