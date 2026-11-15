import { storyboardFeedbackParentReader, storyboardFeedbackScopeSince, storyboardFeedbackFixtureFiles, STORYBOARD_FEEDBACK_BASE } from "./storyboard-feedback-successor-guard.mjs";
import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const PRODUCTION_SAFETY_BASE = "395c473de55446bcc69b4286674b089d0b65e354";
export const PRODUCTION_SAFETY_PROOF = "docs/architecture/production-safety-successor.json";
export const PRODUCTION_SAFETY_CHANGED = [
  "scripts/testing/audit-repair.test.mjs",
  "scripts/testing/ci-chain-consolidated.test.mjs",
  "scripts/testing/convenience.test.mjs",
  "scripts/testing/create-rerun-safety.test.mjs",
  "scripts/testing/input-refresh-safety.test.mjs",
  "scripts/testing/pika-flex-reference.test.mjs",
  "scripts/testing/reference-authority.test.mjs",
  "scripts/testing/storyboard-usage.test.mjs",
  "scripts/testing/usability-completion.test.mjs",
  "scripts/testing/usage-flow.test.mjs",
  "scripts/testing/usage-polish.test.mjs",
  "scripts/usage-polish-successor-guard.mjs",
  "src-tauri/src/application/asset_query_service.rs",
  "src-tauri/src/application/h3_local_import_service.rs",
  "src-tauri/src/application/ports/asset_repository.rs",
  "src-tauri/src/application/product/creation_facade/context.rs",
  "src-tauri/src/application/product/error.rs",
  "src-tauri/src/application/product/selected_export.rs",
  "src-tauri/src/application/production_package_inspector.rs",
  "src-tauri/src/application/project_backup_service.rs",
  "src-tauri/src/commands/product.rs",
  "src-tauri/src/infrastructure/database/repositories/asset.rs",
  "src/app/App.tsx",
  "src/features/create/ShotReviewDialog.tsx",
  "src/features/create/StoryboardShotRows.test.tsx",
  "src/features/create/StoryboardShotRows.tsx",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx",
  "src/features/create/UsageFlowSafety.test.tsx",
  "src/features/runs/RunResults.tsx",
  "src/features/runs/RunsPage.test.tsx",
  "src/features/shots/StoryboardBatchDialog.test.tsx",
  "src/features/shots/StoryboardBatchDialog.tsx",
  "src/product/client.ts",
  "src/product/errors.ts",
  "src/product/runInvalidation.ts",
  "src/product/shotDuration.test.ts",
  "src/product/shotDuration.ts",
  "src/product/transport.ts"
].sort();
export const PRODUCTION_SAFETY_ADDED = [
  "scripts/production-safety-successor-guard.mjs",
  "scripts/testing/production-safety.test.mjs"
].sort();
export const PRODUCTION_SAFETY_CONTRACTS = {
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
export function productionSafetyScopeSince(root, base) {
  const tip = facts(root, PRODUCTION_SAFETY_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...PRODUCTION_SAFETY_CHANGED, ...PRODUCTION_SAFETY_ADDED, PRODUCTION_SAFETY_PROOF,
    ...storyboardFeedbackScopeSince(root, STORYBOARD_FEEDBACK_BASE),
  ])].sort();
}
export function productionSafetyFixtureFiles(root) {
  return [...new Set([...facts(root, PRODUCTION_SAFETY_BASE).paths, ...PRODUCTION_SAFETY_ADDED, PRODUCTION_SAFETY_PROOF, ...storyboardFeedbackFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const successor = storyboardFeedbackParentReader(root, override);
  const workspace = resolve(root), violations = [...successor.violations], parent = facts(workspace, PRODUCTION_SAFETY_BASE);
  const fail = id => violations.push("production-safety-" + id);
  const live = p => canonical(successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(PRODUCTION_SAFETY_PROOF));
    const scope = [...PRODUCTION_SAFETY_CHANGED, ...PRODUCTION_SAFETY_ADDED].sort();
    if (canonical(live(PRODUCTION_SAFETY_PROOF)) !== JSON.stringify(proof, null, 2) + "\n" || JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(["schemaVersion", "checkpoint", "parentHead", "planningParentHead", "contracts", "files"].sort())) fail("proof-bytes");
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "PRODUCTION_SAFETY"
        || proof.parentHead !== PRODUCTION_SAFETY_BASE || proof.planningParentHead !== PRODUCTION_SAFETY_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(PRODUCTION_SAFETY_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", PRODUCTION_SAFETY_BASE]).trim() !== PRODUCTION_SAFETY_BASE) fail("ancestor");
    const allowed = [...scope, PRODUCTION_SAFETY_PROOF].sort();
    const expected = [...parent.paths, ...PRODUCTION_SAFETY_ADDED, PRODUCTION_SAFETY_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of PRODUCTION_SAFETY_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of PRODUCTION_SAFETY_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...PRODUCTION_SAFETY_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...PRODUCTION_SAFETY_ADDED, PRODUCTION_SAFETY_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...PRODUCTION_SAFETY_CHANGED, ...PRODUCTION_SAFETY_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => PRODUCTION_SAFETY_CHANGED.includes(p) ? parent.read(p) : successor.read(p)) };
}
export function productionSafetyParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "productionSafetyParentReader", undefined,
    () => validate(root, undefined), PRODUCTION_SAFETY_PROOF);
}
