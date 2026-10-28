import { h3VedaReferenceParentReader, h3VedaReferenceScopeSince, h3VedaReferenceFixtureFiles, H3_VEDA_REFERENCE_BASE } from './h3-veda-reference-successor-guard.mjs';
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionGit, sessionMemoRead, immutableBlobBytes } from "./boundary-validation-cache.mjs";

export const ENTITY_CATEGORY_LIBRARY_BASE = "3ad01c3191716c70ba3c2d99ac1eb4d210447679";
export const ENTITY_CATEGORY_LIBRARY_PROOF = "docs/architecture/entity-category-library-successor.json";
export const ENTITY_CATEGORY_LIBRARY_CHANGED = [
  "scripts/style-boundary-guard.mjs",
  "scripts/workbench-poll-clock-successor-guard.mjs",
  "src-tauri/src/commands/asset.rs",
  "src-tauri/src/commands/product.rs",
  "src-tauri/src/lib.rs",
  "src/features/library/LibraryFindability.test.tsx",
  "src/features/library/LibraryPage.css",
  "src/features/library/LibraryPage.tsx",
  "src/features/library/libraryModel.ts",
  "src/product/client.test.ts",
  "src/product/client.ts",
  "src/product/transport.ts",
  "src/types/storyEntity.ts"
].sort();
// Runtime Package paths are immutable in this checkpoint.
export const ENTITY_CATEGORY_LIBRARY_RUNTIME = [];
export const ENTITY_CATEGORY_LIBRARY_ADDED = [
  "scripts/entity-category-library-successor-guard.mjs",
  "scripts/testing/entity-category-library.test.mjs",
  "src/features/library/StoryEntityCategorySection.test.tsx",
  "src/features/library/StoryEntityCategorySection.tsx"
].sort();
export const ENTITY_CATEGORY_LIBRARY_CONTRACTS = {
  "schemaChanged": false,
  "backupFormatChanged": false,
  "runtimeBytesChanged": false,
  "historicalProofBytesUnchanged": true,
  "typedIpcChanged": true,
  "backendChanged": true,
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
      blobs.set(p, immutableBlobBytes(header[1], bytes.subarray(start, start + size))); blobIds.set(p, header[1]); offset = start + size + 1;
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
export function entityCategoryLibraryScopeSince(root, base) {
  const tip = facts(root, ENTITY_CATEGORY_LIBRARY_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...ENTITY_CATEGORY_LIBRARY_CHANGED, ...ENTITY_CATEGORY_LIBRARY_ADDED, ENTITY_CATEGORY_LIBRARY_PROOF,
    ...h3VedaReferenceScopeSince(root, H3_VEDA_REFERENCE_BASE),
  ])].sort();
}
export function entityCategoryLibraryFixtureFiles(root) {
  return [...new Set([...facts(root, ENTITY_CATEGORY_LIBRARY_BASE).paths, ...ENTITY_CATEGORY_LIBRARY_ADDED, ENTITY_CATEGORY_LIBRARY_PROOF, ...h3VedaReferenceFixtureFiles(root)])];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, ENTITY_CATEGORY_LIBRARY_BASE);
  const fail = id => violations.push("entity-category-library-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(ENTITY_CATEGORY_LIBRARY_PROOF));
    const scope = [...ENTITY_CATEGORY_LIBRARY_CHANGED, ...ENTITY_CATEGORY_LIBRARY_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "ENTITY_CATEGORY_LIBRARY"
        || proof.parentHead !== ENTITY_CATEGORY_LIBRARY_BASE || proof.planningParentHead !== ENTITY_CATEGORY_LIBRARY_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(ENTITY_CATEGORY_LIBRARY_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", ENTITY_CATEGORY_LIBRARY_BASE]).trim() !== ENTITY_CATEGORY_LIBRARY_BASE) fail("ancestor");
    const allowed = [...scope, ENTITY_CATEGORY_LIBRARY_PROOF].sort();
    const expected = [...parent.paths, ...ENTITY_CATEGORY_LIBRARY_ADDED, ENTITY_CATEGORY_LIBRARY_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p) && !successor.addedPaths.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of ENTITY_CATEGORY_LIBRARY_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of ENTITY_CATEGORY_LIBRARY_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    // Runtime Package bytes remain governed by the historical raw-byte checks.
    for (const p of ENTITY_CATEGORY_LIBRARY_RUNTIME) {
      const bytes = override && Object.hasOwn(override, p) ? Buffer.from(override[p]) : readFileSync(join(workspace, p));
      if (proof.files?.[p]?.afterRawHash !== sha(bytes) || sha(bytes) !== sha(canonical(bytes.toString("utf8")))) fail("runtime-package-bytes:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: [...new Set([...ENTITY_CATEGORY_LIBRARY_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...ENTITY_CATEGORY_LIBRARY_ADDED, ENTITY_CATEGORY_LIBRARY_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...ENTITY_CATEGORY_LIBRARY_CHANGED, ...ENTITY_CATEGORY_LIBRARY_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: sessionMemoRead(workspace, p => ENTITY_CATEGORY_LIBRARY_CHANGED.includes(p) ? parent.read(p) : live(p)) };
}
export function entityCategoryLibraryParentReader(root, override = undefined) {
  const check = () => { const successor = h3VedaReferenceParentReader(root); const historical = validate(root, override, successor); return { ...historical, violations: [...successor.violations, ...historical.violations] }; };
  if (override) return check();
  return cachedBoundary(root, "entityCategoryLibraryParentReader", undefined, check, ENTITY_CATEGORY_LIBRARY_PROOF);
}
