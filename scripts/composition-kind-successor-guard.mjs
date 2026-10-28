import { referenceImagesImportParentReader, referenceImagesImportScopeSince, referenceImagesImportFixtureFiles, REFERENCE_IMAGES_IMPORT_BASE } from './reference-images-import-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const COMPOSITION_KIND_BASE = "ecb180018fcf475c4a78ed7003f91d3dc23435a1";
export const COMPOSITION_KIND_PROOF = "docs/architecture/composition-kind-successor.json";
export const COMPOSITION_KIND_CHANGED = [
  "docs/architecture/folder-batch-import.md",
  "scripts/guard-chain-perf-successor-guard.mjs",
  "src-tauri/src/application/mod.rs",
  "src-tauri/src/application/project_backup_service.rs",
  "src-tauri/src/application/shot_video_input_service.rs",
  "src-tauri/src/domain/mod.rs",
  "src-tauri/src/infrastructure/database/dev048_consistency_e2e.rs",
  "src-tauri/src/infrastructure/database/pool.rs",
  "src-tauri/tests/dev040_safety.rs",
  "src/components/ImageAssetPicker.test.tsx",
  "src/components/ImageAssetPicker.tsx",
  "src/features/create/StoryboardShotRows.tsx",
  "src/features/library/LibraryPage.tsx",
  "src/features/library/StoryEntitiesSection.tsx",
  "src/features/library/StoryEntityCategorySection.test.tsx",
  "src/features/library/StoryEntityCategorySection.tsx",
  "src/features/shots/FolderBatchImportPanel.test.tsx",
  "src/features/shots/FolderBatchImportPanel.tsx",
  "src/types/storyEntity.ts"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const COMPOSITION_KIND_RUNTIME = [];
export const COMPOSITION_KIND_ADDED = [
  "scripts/composition-kind-successor-guard.mjs",
  "scripts/testing/composition-kind.test.mjs",
  "src-tauri/migrations/047_story_entity_composition_kind.sql"
].sort();
export const COMPOSITION_KIND_CONTRACTS = {
  "schemaChanged": true,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": true,
  "compositionKindMigration047": true,
  "pinnedContractStringsPreserved": true
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
      blobs.set(p, immutableBlobBytes(header[1], () => bytes.subarray(start, start + size))); blobIds.set(p, header[1]); offset = start + size + 1;
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
export function compositionKindScopeSince(root, base) {
  const tip = facts(root, COMPOSITION_KIND_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...COMPOSITION_KIND_CHANGED, ...COMPOSITION_KIND_ADDED, COMPOSITION_KIND_PROOF,
    ...referenceImagesImportScopeSince(root, REFERENCE_IMAGES_IMPORT_BASE),
  ])].sort();
}
export function compositionKindFixtureFiles(root) {
  return [...new Set([...facts(root, COMPOSITION_KIND_BASE).paths, ...COMPOSITION_KIND_ADDED, COMPOSITION_KIND_PROOF, ...referenceImagesImportFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, COMPOSITION_KIND_BASE);
  const fail = id => violations.push("composition-kind-" + id);
  // Only an exact, independently validated successor byte version may be projected.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(COMPOSITION_KIND_PROOF));
    const scope = [...COMPOSITION_KIND_CHANGED, ...COMPOSITION_KIND_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "COMPOSITION_KIND"
        || proof.parentHead !== COMPOSITION_KIND_BASE || proof.planningParentHead !== COMPOSITION_KIND_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(COMPOSITION_KIND_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", COMPOSITION_KIND_BASE]).trim() !== COMPOSITION_KIND_BASE) fail("ancestor");
    const allowed = [...scope, COMPOSITION_KIND_PROOF].sort();
    const expected = [...parent.paths, ...COMPOSITION_KIND_ADDED, COMPOSITION_KIND_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of COMPOSITION_KIND_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of COMPOSITION_KIND_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of COMPOSITION_KIND_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...COMPOSITION_KIND_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...COMPOSITION_KIND_ADDED, COMPOSITION_KIND_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...COMPOSITION_KIND_CHANGED, ...COMPOSITION_KIND_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => COMPOSITION_KIND_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function compositionKindParentReader(root, override = undefined) {
  const check = () => { const successor = referenceImagesImportParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "compositionKindParentReader", undefined, check, COMPOSITION_KIND_PROOF);
}
