import { createHash } from "node:crypto";
import { readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionLiveRead, sessionGit } from "./boundary-validation-cache.mjs";

export const PROMPT_SECTIONS_WITHOUT_AT_BASE = "43afa3ae9cb36e4ed3c0cc424837c8e0155acf41";
export const PROMPT_SECTIONS_WITHOUT_AT_PROOF = "docs/architecture/prompt-sections-without-at-successor.json";
export const PROMPT_SECTIONS_WITHOUT_AT_CHANGED = [
  "scripts/testing/cancel-correctness.test.mjs",
  "scripts/testing/delete-integrity.test.mjs",
  "scripts/testing/prompt-section-load.test.mjs",
  "scripts/testing/review-hardening.test.mjs",
  "scripts/testing/single-shot-batch-control.test.mjs",
  "scripts/testing/veda-removed-white-flash.test.mjs",
  "src-tauri/src/application/shot_video_input_service.rs"
].sort();
export const PROMPT_SECTIONS_WITHOUT_AT_ADDED = [
  "scripts/prompt-sections-without-at-successor-guard.mjs",
  "scripts/testing/prompt-sections-without-at.test.mjs"
].sort();
export const PROMPT_SECTIONS_WITHOUT_AT_CONTRACTS = {
  "schemaChanged": false,
  "migrationAdded": false,
  "guardsRelaxed": false,
  "gpuExecuted": false,
  "productionQueueOnlyExecution": true
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
    factsCache.set(key, { paths, read: p => {
      if (!text.has(p)) text.set(p, immutableBlobText(blobIds.get(p), () => blobs.get(p)));
      return text.get(p);
    } });
  }
  return factsCache.get(key);
}
export function promptSectionsWithoutAtScopeSince(root, base) {
  const tip = facts(root, PROMPT_SECTIONS_WITHOUT_AT_BASE), old = facts(root, base);
  return [...new Set([
    ...tip.paths.filter(p => !old.paths.includes(p) || tip.read(p) !== old.read(p)),
    ...PROMPT_SECTIONS_WITHOUT_AT_CHANGED, ...PROMPT_SECTIONS_WITHOUT_AT_ADDED, PROMPT_SECTIONS_WITHOUT_AT_PROOF,
  ])].sort();
}
export function promptSectionsWithoutAtFixtureFiles(root) {
  return [...facts(root, PROMPT_SECTIONS_WITHOUT_AT_BASE).paths, ...PROMPT_SECTIONS_WITHOUT_AT_ADDED, PROMPT_SECTIONS_WITHOUT_AT_PROOF];
}
function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, PROMPT_SECTIONS_WITHOUT_AT_BASE);
  const fail = id => violations.push("prompt-sections-without-at-" + id);
  const live = p => canonical(override && Object.hasOwn(override, p) ? override[p] : sessionLiveRead(workspace, p));
  let proof;
  try {
    proof = JSON.parse(live(PROMPT_SECTIONS_WITHOUT_AT_PROOF));
    const scope = [...PROMPT_SECTIONS_WITHOUT_AT_CHANGED, ...PROMPT_SECTIONS_WITHOUT_AT_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "PROMPT_SECTIONS_WITHOUT_AT"
        || proof.parentHead !== PROMPT_SECTIONS_WITHOUT_AT_BASE || proof.planningParentHead !== PROMPT_SECTIONS_WITHOUT_AT_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(PROMPT_SECTIONS_WITHOUT_AT_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", PROMPT_SECTIONS_WITHOUT_AT_BASE]).trim() !== PROMPT_SECTIONS_WITHOUT_AT_BASE) fail("ancestor");
    const allowed = [...scope, PROMPT_SECTIONS_WITHOUT_AT_PROOF].sort();
    const expected = [...parent.paths, ...PROMPT_SECTIONS_WITHOUT_AT_ADDED, PROMPT_SECTIONS_WITHOUT_AT_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(relevant);
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = [".github/workflows", "src", "scripts", "docs/architecture", "src-tauri/runtime_packages", "src-tauri/src", "src-tauri/migrations", "src-tauri/tests"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    if (changed.some(p => p.startsWith("src-tauri/runtime_packages/"))) fail("historical-runtime-bytes");
    for (const p of PROMPT_SECTIONS_WITHOUT_AT_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of PROMPT_SECTIONS_WITHOUT_AT_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations, changedPaths: PROMPT_SECTIONS_WITHOUT_AT_CHANGED,
    addedPaths: [...PROMPT_SECTIONS_WITHOUT_AT_ADDED, PROMPT_SECTIONS_WITHOUT_AT_PROOF],
    afterHashes: Object.fromEntries([...PROMPT_SECTIONS_WITHOUT_AT_CHANGED, ...PROMPT_SECTIONS_WITHOUT_AT_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])),
    read: p => PROMPT_SECTIONS_WITHOUT_AT_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function promptSectionsWithoutAtParentReader(root, override = undefined) {
  if (override) return validate(root, override);
  return cachedBoundary(root, "promptSectionsWithoutAtParentReader", undefined,
    () => validate(root, undefined), PROMPT_SECTIONS_WITHOUT_AT_PROOF);
}
