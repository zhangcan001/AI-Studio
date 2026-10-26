import { workflowAuthoringRetirementParentReader, workflowAuthoringRetirementScopeSince, workflowAuthoringRetirementFixtureFiles, WORKFLOW_AUTHORING_RETIREMENT_BASE } from './workflow-authoring-retirement-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionGit } from "./boundary-validation-cache.mjs";

export const H3_VEDA_REFERENCE_BASE = "4adf15d2ad4ff0f54f57eb9356d1ebca745832e0";
export const H3_VEDA_REFERENCE_PROOF = "docs/architecture/h3-veda-reference-successor.json";
export const H3_VEDA_REFERENCE_CHANGED = [
  "scripts/entity-category-library-successor-guard.mjs",
  "src-tauri/src/application/builtin_runtime_packages.rs",
  "src-tauri/src/application/minimax_video_product_policy.rs",
  "src-tauri/src/application/product_runtime_scope.rs",
  "src-tauri/src/application/scheduler.rs",
  "src-tauri/tests/dev082_unified_workflow_library.rs",
  "src-tauri/tests/support/minimax_video_policy_regressions.rs"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const H3_VEDA_REFERENCE_RUNTIME = [];
export const H3_VEDA_REFERENCE_ADDED = [
  "docs/architecture/h3-veda-reference-integration.md",
  "scripts/h3-veda-reference-successor-guard.mjs",
  "scripts/testing/h3-veda-reference.test.mjs"
].sort();
export const H3_VEDA_REFERENCE_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "embeddedPackageAdded": true,
  "existingPackagesUnchanged": true,
  "referenceImagesMin": 1,
  "referenceImagesMax": 8,
  "fixedSamplingSteps": 4,
  "gpuExecuted": false
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
export function h3VedaReferenceScopeSince(root, base) {
  const tip = facts(root, H3_VEDA_REFERENCE_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...H3_VEDA_REFERENCE_CHANGED, ...H3_VEDA_REFERENCE_ADDED, H3_VEDA_REFERENCE_PROOF,
    ...workflowAuthoringRetirementScopeSince(root, WORKFLOW_AUTHORING_RETIREMENT_BASE),
  ])].sort();
}
export function h3VedaReferenceFixtureFiles(root) {
  return [...new Set([...facts(root, H3_VEDA_REFERENCE_BASE).paths, ...H3_VEDA_REFERENCE_ADDED, H3_VEDA_REFERENCE_PROOF, ...workflowAuthoringRetirementFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, H3_VEDA_REFERENCE_BASE);
  const fail = id => violations.push("h3-veda-reference-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(H3_VEDA_REFERENCE_PROOF));
    const scope = [...H3_VEDA_REFERENCE_CHANGED, ...H3_VEDA_REFERENCE_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "H3_VEDA_REFERENCE"
        || proof.parentHead !== H3_VEDA_REFERENCE_BASE || proof.planningParentHead !== H3_VEDA_REFERENCE_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(H3_VEDA_REFERENCE_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", H3_VEDA_REFERENCE_BASE]).trim() !== H3_VEDA_REFERENCE_BASE) fail("ancestor");
    const allowed = [...scope, H3_VEDA_REFERENCE_PROOF].sort();
    const expected = [...parent.paths, ...H3_VEDA_REFERENCE_ADDED, H3_VEDA_REFERENCE_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of H3_VEDA_REFERENCE_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of H3_VEDA_REFERENCE_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of H3_VEDA_REFERENCE_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...H3_VEDA_REFERENCE_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...H3_VEDA_REFERENCE_ADDED, H3_VEDA_REFERENCE_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...H3_VEDA_REFERENCE_CHANGED, ...H3_VEDA_REFERENCE_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => H3_VEDA_REFERENCE_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function h3VedaReferenceParentReader(root, override = undefined) {
  const successor = workflowAuthoringRetirementParentReader(root);
  const check = () => { const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "h3VedaReferenceParentReader", undefined, check, H3_VEDA_REFERENCE_PROOF);
}
