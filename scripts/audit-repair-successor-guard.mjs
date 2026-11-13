import { usageFlowParentReader, usageFlowScopeSince, usageFlowFixtureFiles, USAGE_FLOW_BASE } from "./usage-flow-successor-guard.mjs";
import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const AUDIT_REPAIR_BASE = "f64320ae531008bca09dee14715f8a66245bc241";
export const AUDIT_REPAIR_PROOF = "docs/architecture/audit-repair-successor.json";
export const AUDIT_REPAIR_CHANGED = [
  "scripts/testing/ci-chain-consolidated.test.mjs",
  "scripts/testing/create-rerun-safety.test.mjs",
  "scripts/testing/input-refresh-safety.test.mjs",
  "scripts/testing/pika-flex-reference.test.mjs",
  "scripts/testing/reference-authority.test.mjs",
  "scripts/testing/storyboard-usage.test.mjs",
  "scripts/testing/usability-completion.test.mjs",
  "scripts/ui-phase7c-frontend-ci-budget-successor-guard.mjs",
  "scripts/usability-completion-successor-guard.mjs",
  "src-tauri/src/application/product/library_facade/list.rs",
  "src-tauri/src/application/product/library_facade/types.rs",
  "src-tauri/src/application/project_backup_service.rs",
  "src-tauri/src/application/settings_service.rs",
  "src-tauri/src/infrastructure/comfy/client.rs",
  "src-tauri/src/lib.rs",
  "src/components/ImageAssetPicker.tsx",
  "src/features/create/CreateMediaPreview.tsx",
  "src/features/create/CreatePage.css",
  "src/features/create/StoryboardShotRows.test.tsx",
  "src/features/create/StoryboardShotRows.tsx",
  "src/features/create/StoryboardWorkbench.test.tsx",
  "src/features/create/StoryboardWorkbench.tsx",
  "src/features/create/storyboardModel.ts",
  "src/features/library/LibraryPage.tsx",
  "src/features/library/StoryEntityCategorySection.test.tsx",
  "src/features/library/StoryEntityCategorySection.tsx",
  "src/features/shots/FolderBatchImportPanel.test.tsx",
  "src/features/shots/useFolderBatchImportController.ts",
  "src/product/libraryTypes.ts"
].sort();
export const AUDIT_REPAIR_ADDED = [
  "scripts/audit-repair-successor-guard.mjs",
  "scripts/testing/audit-repair.test.mjs",
  "src/components/ImagePickerClassificationSafety.test.tsx",
  "src/features/create/MediaLightboxRecovery.test.tsx",
  "src/features/library/EntityDraftSafety.test.tsx"
].sort();
export const AUDIT_REPAIR_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "historicalRuntimeBytesUnchanged": true,
  "historicalProofBytesUnchanged": true,
  "productionQueueOnlyExecution": true,
  "gpuExecuted": false,
  "guardsRelaxed": false,
  "settingsWritesSerialized": true,
  "inspectionCleanupBounded": true,
  "explicitShotReview": true,
  "draftRecovery": true,
  "saveBeforeGenerate": true,
  "boundedFullProjectSearch": true,
  "projectScopedNavigation": true
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
export function auditRepairScopeSince(root, base) {
  const tip = facts(root, AUDIT_REPAIR_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...AUDIT_REPAIR_CHANGED, ...AUDIT_REPAIR_ADDED, AUDIT_REPAIR_PROOF,
    ...usageFlowScopeSince(root, USAGE_FLOW_BASE),
  ])].sort();
}
export function auditRepairFixtureFiles(root) {
  return [...new Set([...facts(root, AUDIT_REPAIR_BASE).paths, ...AUDIT_REPAIR_ADDED, AUDIT_REPAIR_PROOF, ...usageFlowFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const successor = usageFlowParentReader(root, override);
  const workspace = resolve(root), violations = [...successor.violations], parent = facts(workspace, AUDIT_REPAIR_BASE);
  const fail = id => violations.push("audit-repair-" + id);
  const live = p => canonical(successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(AUDIT_REPAIR_PROOF));
    const scope = [...AUDIT_REPAIR_CHANGED, ...AUDIT_REPAIR_ADDED].sort();
    if (canonical(live(AUDIT_REPAIR_PROOF)) !== JSON.stringify(proof, null, 2) + "\n" || JSON.stringify(Object.keys(proof).sort()) !== JSON.stringify(["schemaVersion", "checkpoint", "parentHead", "planningParentHead", "contracts", "files"].sort())) fail("proof-bytes");
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "AUDIT_REPAIR"
        || proof.parentHead !== AUDIT_REPAIR_BASE || proof.planningParentHead !== AUDIT_REPAIR_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(AUDIT_REPAIR_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", AUDIT_REPAIR_BASE]).trim() !== AUDIT_REPAIR_BASE) fail("ancestor");
    const allowed = [...scope, AUDIT_REPAIR_PROOF].sort();
    const expected = [...parent.paths, ...AUDIT_REPAIR_ADDED, AUDIT_REPAIR_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of AUDIT_REPAIR_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of AUDIT_REPAIR_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...AUDIT_REPAIR_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...AUDIT_REPAIR_ADDED, AUDIT_REPAIR_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...AUDIT_REPAIR_CHANGED, ...AUDIT_REPAIR_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => AUDIT_REPAIR_CHANGED.includes(p) ? parent.read(p) : successor.read(p)) };
}
export function auditRepairParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "auditRepairParentReader", undefined,
    () => validate(root, undefined), AUDIT_REPAIR_PROOF);
}
