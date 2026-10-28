import { entityReferenceAutoApplyParentReader, entityReferenceAutoApplyScopeSince, entityReferenceAutoApplyFixtureFiles, ENTITY_REFERENCE_AUTO_APPLY_BASE } from './entity-reference-auto-apply-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionGit } from "./boundary-validation-cache.mjs";

export const RETIRED_UI_CODE_CLEANUP_BASE = "bf6c4708c1de1d28ed59608d0a48bf7443373b57";
export const RETIRED_UI_CODE_CLEANUP_PROOF = "docs/architecture/retired-ui-code-cleanup-successor.json";
export const RETIRED_UI_CODE_CLEANUP_CHANGED = [
  "scripts/generation-progress-refresh-successor-guard.mjs",
  "src/app/App.navigation.test.ts",
  "src/app/App.tsx",
  "src/app/Phase7Retirement.test.tsx",
  "src/app/ProjectWorkflowBindingUat.test.tsx",
  "src/app/WorkflowNavigationUat.test.tsx",
  "src/features/assets/AssetVideoBatchWorkspace.test.tsx",
  "src/features/assets/AssetVideoBatchWorkspace.tsx",
  "src/features/assets/AssetVideoGenericQueueSubmission.test.tsx",
  "src/features/create/CreatePage.tsx",
  "src/features/generators/GeneratorSettingsPage.tsx",
  "src/features/library/LibraryPage.tsx",
  "src/features/workflow-lab/WorkflowLabPage.tsx"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const RETIRED_UI_CODE_CLEANUP_RUNTIME = [];
export const RETIRED_UI_CODE_CLEANUP_ADDED = [
  "docs/architecture/retired-ui-code-cleanup.md",
  "scripts/retired-ui-code-cleanup-successor-guard.mjs",
  "scripts/testing/retired-ui-code-cleanup.test.mjs"
].sort();
export const RETIRED_UI_CODE_CLEANUP_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "queueAuthorityChanged": false,
  "gpuExecuted": false,
  "retiredAuthoringCallbacksRemoved": true,
  "legacyRouteNavigationPreserved": true,
  "currentGeneratorBindingsPreserved": true,
  "safetyAndErrorFeedbackPreserved": true
};
const canonical = v => v.replaceAll("\r\n", "\n");
const sha = v => createHash("sha256").update(v).digest("hex");
const relevant = p => p === "vite.config.ts" || /^(\.github\/workflows\/|src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p);
const git = (root, args) => sessionGit(root, args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
const factsCache = new Map();
function facts(root, base) {
  const key = JSON.stringify([immutableGitStoreIdentity(root), base]);
  if (!factsCache.has(key)) {
    const paths = immutableGit("git", ["ls-tree", "-r", "--name-only", base], { cwd: root, encoding: "utf8" })
      .trim().split(/\r?\n/).filter(relevant).sort();
    const bytes = immutableGit("git", ["cat-file", "--batch"], {
      cwd: root, input: paths.map(p => base + ":" + p + "\n").join(""), maxBuffer: 64 * 1024 * 1024,
    });
    const blobs = new Map(), blobIds = new Map(); let offset = 0;
    for (const p of paths) {
      const end = bytes.indexOf(10, offset), header = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
      if (!header) throw Error("immutable blob header");
      const start = end + 1, size = Number(header[2]);
      if (bytes[start + size] !== 10) throw Error("immutable blob boundary");
      blobs.set(p, bytes.subarray(start, start + size)); blobIds.set(p, header[1]); offset = start + size + 1;
    }
    if (offset !== bytes.length) throw Error("immutable trailing bytes");
    const text = new Map();
    factsCache.set(key, { paths, raw: p => blobs.get(p), read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function retiredUiCodeCleanupScopeSince(root, base) {
  const tip = facts(root, RETIRED_UI_CODE_CLEANUP_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...RETIRED_UI_CODE_CLEANUP_CHANGED, ...RETIRED_UI_CODE_CLEANUP_ADDED, RETIRED_UI_CODE_CLEANUP_PROOF,
    ...entityReferenceAutoApplyScopeSince(root, ENTITY_REFERENCE_AUTO_APPLY_BASE),
  ])].sort();
}
export function retiredUiCodeCleanupFixtureFiles(root) {
  return [...new Set([...facts(root, RETIRED_UI_CODE_CLEANUP_BASE).paths, ...RETIRED_UI_CODE_CLEANUP_ADDED, RETIRED_UI_CODE_CLEANUP_PROOF, ...entityReferenceAutoApplyFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, RETIRED_UI_CODE_CLEANUP_BASE);
  const fail = id => violations.push("retired-ui-code-cleanup-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(RETIRED_UI_CODE_CLEANUP_PROOF));
    const scope = [...RETIRED_UI_CODE_CLEANUP_CHANGED, ...RETIRED_UI_CODE_CLEANUP_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "RETIRED_UI_CODE_CLEANUP"
        || proof.parentHead !== RETIRED_UI_CODE_CLEANUP_BASE || proof.planningParentHead !== RETIRED_UI_CODE_CLEANUP_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(RETIRED_UI_CODE_CLEANUP_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", RETIRED_UI_CODE_CLEANUP_BASE]).trim() !== RETIRED_UI_CODE_CLEANUP_BASE) fail("ancestor");
    const allowed = [...scope, RETIRED_UI_CODE_CLEANUP_PROOF].sort();
    const expected = [...parent.paths, ...RETIRED_UI_CODE_CLEANUP_ADDED, RETIRED_UI_CODE_CLEANUP_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of RETIRED_UI_CODE_CLEANUP_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of RETIRED_UI_CODE_CLEANUP_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of RETIRED_UI_CODE_CLEANUP_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...RETIRED_UI_CODE_CLEANUP_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...RETIRED_UI_CODE_CLEANUP_ADDED, RETIRED_UI_CODE_CLEANUP_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...RETIRED_UI_CODE_CLEANUP_CHANGED, ...RETIRED_UI_CODE_CLEANUP_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => RETIRED_UI_CODE_CLEANUP_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function retiredUiCodeCleanupParentReader(root, override = undefined) {
  const check = () => { const successor = entityReferenceAutoApplyParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "retiredUiCodeCleanupParentReader", undefined, check, RETIRED_UI_CODE_CLEANUP_PROOF);
}
