import { taskCancelDeleteParentReader, taskCancelDeleteScopeSince, taskCancelDeleteFixtureFiles, TASK_CANCEL_DELETE_BASE } from './task-cancel-delete-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit } from "./boundary-validation-cache.mjs";

export const H3_PROMPT_SECTIONS_BASE = "2e9201f778987d46652b2bd989e41eb1bf05d21c";
export const H3_PROMPT_SECTIONS_PROOF = "docs/architecture/h3-prompt-sections-successor.json";
export const H3_PROMPT_SECTIONS_CHANGED = [
  "docs/architecture/folder-batch-import.md",
  "scripts/h3-composition-layout-successor-guard.mjs",
  "src-tauri/src/application/mod.rs",
  "src-tauri/src/application/ports/mod.rs",
  "src-tauri/src/application/project_backup_service.rs",
  "src-tauri/src/application/shot_video_input_service.rs",
  "src-tauri/src/commands/mod.rs",
  "src-tauri/src/commands/product.rs",
  "src-tauri/src/domain/mod.rs",
  "src-tauri/src/infrastructure/database/dev048_consistency_e2e.rs",
  "src-tauri/src/infrastructure/database/pool.rs",
  "src-tauri/src/infrastructure/database/repositories/mod.rs",
  "src-tauri/src/infrastructure/database/repositories/project_backup.rs",
  "src-tauri/src/lib.rs",
  "src-tauri/tests/dev040_safety.rs",
  "src-tauri/tests/dev055_release_compatibility.rs",
  "src-tauri/tests/support/minimax_video_input_regressions.rs",
  "src/features/create/StoryboardShotRows.test.tsx",
  "src/features/create/StoryboardShotRows.tsx",
  "src/features/library/StoryEntitiesSection.tsx",
  "src/features/library/StoryEntityCategorySection.test.tsx",
  "src/features/production/ProductionQueueDrawer.tsx",
  "src/product/client.ts",
  "src/product/promptLanguage.test.ts",
  "src/product/promptLanguage.ts",
  "src/product/transport.ts",
  "src/types/storyEntity.ts"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const H3_PROMPT_SECTIONS_RUNTIME = [];
export const H3_PROMPT_SECTIONS_ADDED = [
  "scripts/h3-prompt-sections-successor-guard.mjs",
  "scripts/testing/h3-prompt-sections.test.mjs",
  "src-tauri/migrations/049_shot_prompt_sections.sql"
].sort();
export const H3_PROMPT_SECTIONS_CONTRACTS = {
  "schemaChanged": true,
  "backupFormatChanged": true,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "productImplementationChanged": true,
  "shotDurationCapSeconds": 11,
  "compositionAsShotPlanningReference": true,
  "officialRetentionMarkersOnly": true,
  "englishPromptGuide": true,
  "englishPromptHintNonBlocking": true,
  "shotPromptSectionsMigration049": true,
  "nonBlockingWritingHints": true,
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
export function h3PromptSectionsScopeSince(root, base) {
  const tip = facts(root, H3_PROMPT_SECTIONS_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...H3_PROMPT_SECTIONS_CHANGED, ...H3_PROMPT_SECTIONS_ADDED, H3_PROMPT_SECTIONS_PROOF,
    ...taskCancelDeleteScopeSince(root, TASK_CANCEL_DELETE_BASE),
  ])].sort();
}
export function h3PromptSectionsFixtureFiles(root) {
  return [...new Set([...facts(root, H3_PROMPT_SECTIONS_BASE).paths, ...H3_PROMPT_SECTIONS_ADDED, H3_PROMPT_SECTIONS_PROOF, ...taskCancelDeleteFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, H3_PROMPT_SECTIONS_BASE);
  const fail = id => violations.push("h3-prompt-sections-" + id);
  // Only an exact, independently validated successor byte version may be projected.
  const live = p => {
    const value = override && Object.hasOwn(override, p) ? canonical(override[p]) : undefined;
    return value !== undefined && !(successor.violations.length === 0 && sha(value) === successor.afterHashes[p]) ? value : canonical(successor.read(p));
  };
  let proof;
  try {
    proof = JSON.parse(live(H3_PROMPT_SECTIONS_PROOF));
    const scope = [...H3_PROMPT_SECTIONS_CHANGED, ...H3_PROMPT_SECTIONS_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "H3_PROMPT_SECTIONS"
        || proof.parentHead !== H3_PROMPT_SECTIONS_BASE || proof.planningParentHead !== H3_PROMPT_SECTIONS_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(H3_PROMPT_SECTIONS_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", H3_PROMPT_SECTIONS_BASE]).trim() !== H3_PROMPT_SECTIONS_BASE) fail("ancestor");
    const allowed = [...scope, H3_PROMPT_SECTIONS_PROOF].sort();
    const expected = [...parent.paths, ...H3_PROMPT_SECTIONS_ADDED, H3_PROMPT_SECTIONS_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of H3_PROMPT_SECTIONS_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of H3_PROMPT_SECTIONS_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of H3_PROMPT_SECTIONS_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...H3_PROMPT_SECTIONS_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...H3_PROMPT_SECTIONS_ADDED, H3_PROMPT_SECTIONS_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...H3_PROMPT_SECTIONS_CHANGED, ...H3_PROMPT_SECTIONS_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => H3_PROMPT_SECTIONS_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function h3PromptSectionsParentReader(root, override = undefined) {
  const successor = taskCancelDeleteParentReader(root);
  const check = () => { const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "h3PromptSectionsParentReader", undefined, check, H3_PROMPT_SECTIONS_PROOF);
}
