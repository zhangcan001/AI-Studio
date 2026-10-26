import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { cachedBoundary, immutableGit, immutableGitStoreIdentity, immutableBlobText, sessionGit } from "./boundary-validation-cache.mjs";

import { uiPhase7BR1MetadataParentReader, phase7BR1MetadataScopeSince, phase7BR1MetadataFixtureFiles } from "./ui-phase7b-r1-metadata-successor-guard.mjs";

export const BACKUP_PROMPT_BASE = "f43bb2175f1e375b763a29ae928dfa148279e9cd";
export const BACKUP_PROMPT_PROOF = "docs/architecture/ui-phase7b-backup-prompt-successor.json";
export const BACKUP_PROMPT_CHANGED = [
  "src-tauri/src/application/project_backup_service.rs",
  "src-tauri/src/infrastructure/database/repositories/project_backup.rs",
  "scripts/ui-phase7b-workspace-hydration-successor-guard.mjs",
].sort();
export const BACKUP_PROMPT_ADDED = [
  "scripts/ui-phase7b-backup-prompt-successor-guard.mjs",
  "scripts/testing/ui-phase7b-backup-prompt.test.mjs",
].sort();
export const BACKUP_PROMPT_CONTRACTS = {
  schemaChanged: false, backupFormatChanged: false, backupVersion: 21,
  exportDefaultProjectionChanged: true, strictReaderUnchanged: true,
  sourceDatabaseNotWritten: true, persistedPromptsPreserved: true,
  historicalProofBytesUnchanged: true, runtimeBytesUnchanged: true,
  singleQueue: true, gpuExecuted: false,
};
const canonical = v => v.replaceAll("\r\n", "\n");
const sha = v => createHash("sha256").update(v).digest("hex");
const relevant = p => p === "vite.config.ts" || /^(src\/|src-tauri\/|scripts\/|docs\/architecture\/)/.test(p);
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
export function phase7BBackupPromptScopeSince(root, base) {
  return phase7BR1MetadataScopeSince(root, base);
}
export function phase7BBackupPromptFixtureFiles(root) {
  return phase7BR1MetadataFixtureFiles(root);
}

function files(root, dir) {
  return readdirSync(join(root, dir), { withFileTypes: true }).flatMap(e => {
    const p = dir + "/" + e.name;
    if (e.isSymbolicLink()) throw Error("symbolic source: " + p);
    return e.isDirectory() ? files(root, p) : [p];
  });
}
function validate(root, override, successor) {
  const workspace = resolve(root), violations = [], parent = facts(workspace, BACKUP_PROMPT_BASE);
  const fail = id => violations.push("ui-phase7b-backup-prompts-" + id);
  const live = p => canonical(successor.read(p));
  let proof;
  try {
    proof = JSON.parse(live(BACKUP_PROMPT_PROOF));
    const scope = [...BACKUP_PROMPT_CHANGED, ...BACKUP_PROMPT_ADDED].sort();
    if (proof.schemaVersion !== 1 || proof.checkpoint !== "UI_PHASE7B_BACKUP_PROMPTS"
        || proof.parentHead !== BACKUP_PROMPT_BASE || proof.planningParentHead !== BACKUP_PROMPT_BASE
        || JSON.stringify(proof.contracts) !== JSON.stringify(BACKUP_PROMPT_CONTRACTS)
        || JSON.stringify(Object.keys(proof.files ?? {}).sort()) !== JSON.stringify(scope)) fail("proof-contract");
    if (git(workspace, ["rev-parse", BACKUP_PROMPT_BASE]).trim() !== BACKUP_PROMPT_BASE) fail("ancestor");
    const allowed = [...scope, BACKUP_PROMPT_PROOF].sort();
    const expected = [...parent.paths, ...BACKUP_PROMPT_ADDED, BACKUP_PROMPT_PROOF].sort();
    const inventory = git(workspace, ["ls-files", "--cached", "--others", "--exclude-standard"])
      .trim().split(/\r?\n/).filter(p => relevant(p) && !successor.addedPaths.includes(p));
    if (JSON.stringify([...new Set(inventory)].sort()) !== JSON.stringify(expected)) fail("inventory");
    const actualFiles = ["src", "scripts", "docs/architecture"].flatMap(d => files(workspace, d));
    if (actualFiles.some(p => !successor.addedPaths.includes(p) && !expected.includes(p))) fail("illegal-file");
    const changed = parent.paths.filter(p => live(p) !== parent.read(p));
    const added = inventory.filter(p => !parent.paths.includes(p));
    if (JSON.stringify([...new Set([...changed, ...added])].sort()) !== JSON.stringify(allowed)) fail("unapproved-source");
    for (const p of BACKUP_PROMPT_CHANGED) {
      if (proof.files?.[p]?.beforeHash !== sha(parent.read(p)) || proof.files?.[p]?.afterHash !== sha(live(p))) fail("changed:" + p);
    }
    for (const p of BACKUP_PROMPT_ADDED) {
      if (parent.paths.includes(p) || proof.files?.[p]?.beforeHash !== null || proof.files?.[p]?.afterHash !== sha(live(p))) fail("added:" + p);
    }
    for (const p of Object.keys(override ?? {}).filter(p => relevant(p) && !allowed.includes(p))) {
      if (!parent.paths.includes(p) || live(p) !== parent.read(p)) fail("undeclared:" + p);
    }
    const service = "src-tauri/src/application/project_backup_service.rs";
    const production = text => text.split("#[cfg(test)]\nmod tests")[0];
    if (production(live(service)) !== production(parent.read(service))) fail("strict-reader-changed");
    const repository = "src-tauri/src/infrastructure/database/repositories/project_backup.rs";
    const outsideProjection = text => {
      const start = text.indexOf("fn materialize_legacy_shot_stage_prompts(");
      const end = text.indexOf("\nasync fn query_shot_reference_assets(", start);
      if (start < 0 || end < 0) throw Error("projection boundary");
      return text.slice(0, start) + text.slice(end);
    };
    if (outsideProjection(live(repository)) !== outsideProjection(parent.read(repository))) fail("unrelated-repository-change");
    const runtime = parent.paths.filter(p => p.startsWith("src-tauri/runtime_packages/"));
    const aggregate = read => sha(JSON.stringify(runtime.map(p => [p, sha(read(p))])));
    const profiles = JSON.parse(parent.read("docs/architecture/minimax-video-phase2.json")).runtimeByteBaselines;
    const profile = process.env.AI_STUDIO_PHASE2_RUNTIME_BYTE_PROFILE ?? "git";
    if (aggregate(p => parent.raw(p)) !== profiles.git
        || aggregate(p => readFileSync(join(workspace, p))) !== profiles[profile]) fail("runtime-package-bytes");
  } catch (error) { fail("validation:" + String(error).slice(0, 180)); }
  return { violations: [...successor.violations, ...violations], changedPaths: [...new Set([...BACKUP_PROMPT_CHANGED, ...successor.changedPaths])].sort(),
    addedPaths: [...new Set([...BACKUP_PROMPT_ADDED, BACKUP_PROMPT_PROOF, ...successor.addedPaths])].sort(),
    afterHashes: { ...Object.fromEntries([...BACKUP_PROMPT_CHANGED, ...BACKUP_PROMPT_ADDED].map(p => [p, proof?.files?.[p]?.afterHash ?? ""])), ...successor.afterHashes },
    read: p => BACKUP_PROMPT_CHANGED.includes(p) ? parent.read(p) : live(p) };
}
export function uiPhase7BBackupPromptParentReader(root, override = undefined) {
  const successor = uiPhase7BR1MetadataParentReader(root, override);
  if (successor.violations.length) return { ...successor, violations: [...successor.violations, ...(successor.violations.includes("ui-phase7b-r1-metadata-runtime-package-bytes") ? ["ui-phase7b-backup-prompts-runtime-package-bytes"] : [])] };
  if (override) return validate(root, override, successor);
  return cachedBoundary(root, "uiPhase7BBackupPromptParentReader", undefined,
    () => validate(root, undefined, successor), BACKUP_PROMPT_PROOF);
}
