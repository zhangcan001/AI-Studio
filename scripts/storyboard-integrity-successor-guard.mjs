import { runRecoveryPolishParentReader, runRecoveryPolishScopeSince, runRecoveryPolishFixtureFiles, RUN_RECOVERY_POLISH_BASE } from "./run-recovery-polish-successor-guard.mjs";
import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const STORYBOARD_INTEGRITY_BASE = "1fd0532a105e650d904cdfa5b044a3a0ef82fee3";
export const STORYBOARD_INTEGRITY_PROOF = "docs/architecture/storyboard-integrity-successor.json";
export const STORYBOARD_INTEGRITY_CHANGED = [
  "scripts/storyboard-feedback-successor-guard.mjs",
  "scripts/testing/audit-repair.test.mjs",
  "scripts/testing/ci-chain-consolidated.test.mjs",
  "scripts/testing/convenience.test.mjs",
  "scripts/testing/create-rerun-safety.test.mjs",
  "scripts/testing/input-refresh-safety.test.mjs",
  "scripts/testing/pika-flex-reference.test.mjs",
  "scripts/testing/production-safety.test.mjs",
  "scripts/testing/reference-authority.test.mjs",
  "scripts/testing/storyboard-feedback.test.mjs",
  "scripts/testing/storyboard-usage.test.mjs",
  "scripts/testing/usability-completion.test.mjs",
  "scripts/testing/usage-flow.test.mjs",
  "scripts/testing/usage-polish.test.mjs",
  "src-tauri/src/application/product/creation_facade/context.rs",
  "src-tauri/src/application/shot_video_input_service.rs",
  "src-tauri/src/infrastructure/database/repositories/mod.rs",
  "src-tauri/tests/product_library_contract.rs",
  "src-tauri/tests/support/creation_context_contract.rs",
  "src/app/App.tsx",
  "src/app/M2CreateLibraryNavigation.test.tsx",
  "src/features/assets/AssetWorkspace.test.tsx",
  "src/features/assets/AssetWorkspace.tsx",
  "src/features/create/StoryboardShotRows.test.tsx",
  "src/features/create/StoryboardShotRows.tsx",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx",
  "src/features/create/storyboardModel.ts",
  "src/features/library/LibraryDetail.tsx",
  "src/features/library/LibraryFindability.test.tsx",
  "src/features/library/LibraryPage.tsx",
  "src/features/library/StoryEntityCategorySection.test.tsx",
  "src/features/library/StoryEntityCategorySection.tsx",
  "src/features/shots/ShotBulkConfigPanel.tsx",
  "src/features/shots/StoryboardBatchDialog.test.tsx",
  "src/product/types.ts",
  "src/stores/studioStore.test.ts",
  "src/stores/studioStore.ts"
].sort();
export const STORYBOARD_INTEGRITY_ADDED = [
  "scripts/storyboard-integrity-successor-guard.mjs",
  "scripts/testing/storyboard-integrity.test.mjs"
].sort();
export const STORYBOARD_INTEGRITY_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "historicalRuntimeBytesUnchanged": true,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "guardsRelaxed": false,
  "explicitSelectionAndReview": true,
  "boundedWorkbenchPaging": true,
  "noImplicitStart": true
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
      blobs.set(p, immutableBlobBytes(header[1], bytes.subarray(start, start + size))); blobIds.set(p, header[1]); offset = start + size + 1;
    }
    if (offset !== bytes.length) throw Error("immutable trailing bytes");
    const text = new Map();
    factsCache.set(key, { paths, read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function storyboardIntegrityScopeSince(root, base) {
  const tip = facts(root, STORYBOARD_INTEGRITY_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...STORYBOARD_INTEGRITY_CHANGED, ...STORYBOARD_INTEGRITY_ADDED, STORYBOARD_INTEGRITY_PROOF,
    ...runRecoveryPolishScopeSince(root, RUN_RECOVERY_POLISH_BASE),
  ])].sort();
}
export function storyboardIntegrityFixtureFiles(root) {
  return [...new Set([...facts(root, STORYBOARD_INTEGRITY_BASE).paths, ...STORYBOARD_INTEGRITY_ADDED, STORYBOARD_INTEGRITY_PROOF, ...runRecoveryPolishFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const successor = runRecoveryPolishParentReader(root, override);
  const workspace = resolve(root), violations = [...successor.violations], parent = facts(workspace, STORYBOARD_INTEGRITY_BASE);
  const fail = id => violations.push("storyboard-integrity-" + id);
  const live = p => canonical(successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(STORYBOARD_INTEGRITY_PROOF));
    const scope = [...STORYBOARD_INTEGRITY_CHANGED, ...STORYBOARD_INTEGRITY_ADDED].sort();
    if (canonical(live(STORYBOARD_INTEGRITY_PROOF)) !== JSON.stringify(proof, null, 2) + "\n" || JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(["schemaVersion", "checkpoint", "parentHead", "planningParentHead", "contracts", "files"].sort())) fail("proof-bytes");
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "STORYBOARD_INTEGRITY"
        || proof.parentHead !== STORYBOARD_INTEGRITY_BASE || proof.planningParentHead !== STORYBOARD_INTEGRITY_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(STORYBOARD_INTEGRITY_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", STORYBOARD_INTEGRITY_BASE]).trim() !== STORYBOARD_INTEGRITY_BASE) fail("ancestor");
    const allowed = [...scope, STORYBOARD_INTEGRITY_PROOF].sort();
    const expected = [...parent.paths, ...STORYBOARD_INTEGRITY_ADDED, STORYBOARD_INTEGRITY_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of STORYBOARD_INTEGRITY_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of STORYBOARD_INTEGRITY_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...STORYBOARD_INTEGRITY_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...STORYBOARD_INTEGRITY_ADDED, STORYBOARD_INTEGRITY_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...STORYBOARD_INTEGRITY_CHANGED, ...STORYBOARD_INTEGRITY_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => STORYBOARD_INTEGRITY_CHANGED.includes(p) ? parent.read(p) : successor.read(p)) };
}
export function storyboardIntegrityParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "storyboardIntegrityParentReader", undefined,
    () => validate(root, undefined), STORYBOARD_INTEGRITY_PROOF);
}
