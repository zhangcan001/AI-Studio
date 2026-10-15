import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity } from "./boundary-validation-cache.mjs";

export const R1_METADATA_BASE = "0c3be97a88d5458fc16851bfa19dc10172007886";
export const R1_METADATA_PROOF = "docs/architecture/ui-phase7b-r1-metadata-successor.json";
export const R1_METADATA_CHANGED = [
  "scripts/ui-phase7b-backup-prompt-successor-guard.mjs",
  "src-tauri/Cargo.toml",
  "src-tauri/src/application/minimax_video_product_policy.rs",
  "src-tauri/src/application/product/creation_facade/context.rs",
  "src-tauri/src/application/product/h3_resolution.rs",
  "src-tauri/src/application/product/creation_facade/submission.rs",
  "src-tauri/src/application/product/run_facade.rs",
  "src-tauri/src/application/product/run_facade/detail.rs",
  "src-tauri/src/application/production_queue_service.rs",
  "src-tauri/src/application/production_start_admission_service.rs",
  "src-tauri/src/application/project_backup_service.rs",
  "src-tauri/src/application/workflow_onboarding_service.rs",
  "src-tauri/src/application/workflow_registry_service.rs",
  "src-tauri/src/commands/product.rs",
  "src-tauri/src/infrastructure/database/repositories/project_backup.rs",
  "src-tauri/src/lib.rs",
  "src-tauri/tests/dev061b_queue_recovery.rs",
  "src-tauri/tests/support/creation_submission_contract.rs",
  "src/app/FeatureLifecycle.test.tsx",
  "src/app/FrontendConsolidation.test.tsx",
  "src/app/NormalProductPages.tsx",
  "src/app/v3/AppShellV3.css",
  "src/features/create/CreateController.ts",
  "src/features/create/CreateInputs.tsx",
  "src/features/create/CreateMediaPreview.tsx",
  "src/features/create/CreatePage.css",
  "src/features/create/CreatePage.test.tsx",
  "src/features/create/CreatePage.tsx",
  "src/features/create/CreateReadinessLifecycle.test.tsx",
  "src/features/create/CreateResults.tsx",
  "src/features/create/CreateVideoWorkspace.test.tsx",
  "src/features/create/CreateVideoWorkspace.tsx",
  "src/features/create/createModel.ts",
  "src/features/create/usePersistentVideoInputs.test.tsx",
  "src/features/create/usePersistentVideoInputs.ts",
  "src/features/generators/GeneratorSettingsPage.test.tsx",
  "src/features/generators/GeneratorSettingsPage.tsx",
  "src/features/library/LibraryPage.test.tsx",
  "src/features/runs/RunsController.ts",
  "src/features/tools/LocalToolHub.test.tsx",
].sort();
export const R1_METADATA_ADDED = [
  "scripts/testing/ui-phase7b-r1-metadata.test.mjs",
  "scripts/ui-phase7b-r1-metadata-successor-guard.mjs",
  "src/features/create/CandidateThumbnail.test.tsx",
  "src/features/create/CreateComposer.test.tsx",
  "src/features/create/CreateComposer.tsx",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx",
  "src/features/create/composerTestUtils.ts",
  "src/features/create/storyboardModel.ts",
].sort();
export const R1_METADATA_CONTRACTS = {
  schemaChanged: false, backupFormatChanged: true, backupVersion: 21,
  optionalExecutionMetadata: true, legacyUnknownNonExecutable: true,
  restoredOwnerRemapped: true, typedMediaRemapped: true,
  sourceDatabaseNotWritten: true, historicalProofBytesUnchanged: true,
  runtimeBytesUnchanged: true, singleQueue: true, gpuExecuted: false, nativeDelayDebugOnly: true,
};
const canonical = v => v.replaceAll("\r\n", "\n");
const sha = v => createHash("sha256").update(v).digest("hex");
const relevant = p => p === "vite.config.ts" || /^(src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p);
const git = (root, args) => execFileSync("git", args, { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
const factsCache = new Map();
function facts(root, base) {
  const key = JSON.stringify([immutableGitStoreIdentity(root), base]);
  if (!factsCache.has(key)) {
    const paths = immutableGit("git", ["ls-tree", "-r", "--name-only", base], { cwd: root, encoding: "utf8" })
      .trim().split(/\r?\n/).filter(relevant).sort();
    const bytes = immutableGit("git", ["cat-file", "--batch"], {
      cwd: root, input: paths.map(p => base + ":" + p + "\n").join(""), maxBuffer: 64 * 1024 * 1024,
    });
    const blobs = new Map(); let offset = 0;
    for (const p of paths) {
      const end = bytes.indexOf(10, offset), header = /^([a-f0-9]{40}) blob (\d+)$/.exec(bytes.subarray(offset, end).toString());
      if (!header) throw Error("immutable blob header");
      const start = end + 1, size = Number(header[2]);
      if (bytes[start + size] !== 10) throw Error("immutable blob boundary");
      blobs.set(p, bytes.subarray(start, start + size)); offset = start + size + 1;
    }
    if (offset !== bytes.length) throw Error("immutable trailing bytes");
    const text = new Map();
    factsCache.set(key, { paths, raw: p => blobs.get(p), read: p => {
      if (!text.has(p)) text.set(p, canonical(blobs.get(p).toString("utf8")));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function phase7BR1MetadataScopeSince(root, base) {
  const tip = facts(root, R1_METADATA_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...R1_METADATA_CHANGED, ...R1_METADATA_ADDED, R1_METADATA_PROOF,
  ])].sort();
}
export function phase7BR1MetadataFixtureFiles(root) {
  return [...facts(root, R1_METADATA_BASE).paths, ...R1_METADATA_ADDED, R1_METADATA_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, R1_METADATA_BASE);
  const fail = id => violations.push("ui-phase7b-r1-metadata-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : readFileSync(join(workspace, p), "utf8"));
  let proof;
  try {
    proof = JSON.parse(live(R1_METADATA_PROOF));
    const scope = [...R1_METADATA_CHANGED, ...R1_METADATA_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "UI_PHASE7B_R1_METADATA"
        || proof.parentHead !== R1_METADATA_BASE || proof.planningParentHead !== R1_METADATA_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(R1_METADATA_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", R1_METADATA_BASE]).trim() !== R1_METADATA_BASE) fail("ancestor");
    const allowed = [...scope, R1_METADATA_PROOF].sort();
    const expected = [...parent.paths, ...R1_METADATA_ADDED, R1_METADATA_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = ["src", "scripts", "docs/architecture"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    for (const p of R1_METADATA_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of R1_METADATA_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
    const runtime = parent.paths.filter(p => p.startsWith("src-tauri/runtime_packages/"));
    const aggregate = read => sha(JSON.stringify(runtime.map(p => [p, sha(read(p))])));
    const profiles = JSON.parse(parent.read("docs/architecture/minimax-video-phase2.json")).runtimeByteBaselines;
    const profile = process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE ?? "git";
    if (aggregate(p => parent.raw(p)) !== profiles.git
        || aggregate(p => readFileSync(join(workspace, p))) !== profiles[profile]) fail("runtime-package-bytes");
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: R1_METADATA_CHANGED,
    addedPaths: [...R1_METADATA_ADDED, R1_METADATA_PROOF],
    afterHashes: Object.fromEntries([...R1_METADATA_CHANGED, ...R1_METADATA_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: p => R1_METADATA_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function uiPhase7BR1MetadataParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "uiPhase7BR1MetadataParentReader", undefined,
    () => validate(root, undefined), R1_METADATA_PROOF);
}
