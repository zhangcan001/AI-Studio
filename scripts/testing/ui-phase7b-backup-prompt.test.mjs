// @vitest-environment node
import { it, expect } from "vitest";
import { readFileSync, writeFileSync, copyFileSync, mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { BACKUP_PROMPT_BASE, BACKUP_PROMPT_PROOF, uiPhase7BBackupPromptParentReader } from "../ui-phase7b-backup-prompt-successor-guard.mjs";
import { uiPhase7BWorkspaceHydrationParentReader, HYDRATION_PROOF } from "../ui-phase7b-workspace-hydration-successor-guard.mjs";

it("validates the backup correction before projecting the frozen f43 hydration checkpoint", () => {
  const view = uiPhase7BBackupPromptParentReader(".");
  expect(view.violations).toEqual([]);
  expect(uiPhase7BWorkspaceHydrationParentReader(".").violations).toEqual([]);
  const path = "src-tauri/src/infrastructure/database/repositories/project_backup.rs";
  expect(view.read(path)).toEqual(execFileSync("git", ["show", BACKUP_PROMPT_BASE + ":" + path], { encoding: "utf8" }));
}, 120000);

it("rejects forged identity/hashes, protected reader edits and undeclared changes", () => {
  const proof = JSON.parse(readFileSync(BACKUP_PROMPT_PROOF, "utf8"));
  const repository = "src-tauri/src/infrastructure/database/repositories/project_backup.rs";
  const service = "src-tauri/src/application/project_backup_service.rs";
  const cases = [
    { [BACKUP_PROMPT_PROOF]: JSON.stringify({ ...proof, parentHead: "forged" }) },
    { [BACKUP_PROMPT_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [repository]: { ...proof.files[repository], afterHash: "0".repeat(64) } } }) },
    { [BACKUP_PROMPT_PROOF]: "" },
    { [repository]: readFileSync(repository, "utf8") + "\n// undeclared SQL change\n" },
    { [service]: readFileSync(service, "utf8").replace('stage_prompt_counts.get(shot.id.as_str()).copied() != Some(2)', 'false') },
    { [HYDRATION_PROOF]: readFileSync(HYDRATION_PROOF, "utf8") + "invalid" },
    { "src-tauri/migrations/999_unauthorized.sql": "SELECT 1;" },
  ];
  for (const override of cases) expect(uiPhase7BBackupPromptParentReader(".", override).violations.length).toBeGreaterThan(0);
  expect(uiPhase7BBackupPromptParentReader(".").violations).toEqual([]);
}, 120000);

it("invalidates cached live fixtures on runtime changes, deletion and illegal additions; restoration passes", () => {
  const fixture = mkdtempSync(join(tmpdir(), "phase7b-backup-proof-"));
  try {
    const paths = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard"], { encoding: "utf8" }).trim().split(/\r?\n/);
    for (const path of paths) {
      mkdirSync(dirname(join(fixture, path)), { recursive: true });
      copyFileSync(path, join(fixture, path));
    }
    execFileSync("git", ["init", "-q", fixture]);
    writeFileSync(join(fixture, ".git/objects/info/alternates"), resolve(".git/objects").replaceAll("\\", "/") + "\n");
    execFileSync("git", ["config", "core.autocrlf", "true"], { cwd: fixture });
    const validate = () => uiPhase7BBackupPromptParentReader(fixture).violations;
    expect(validate()).toEqual([]);
    const path = paths.find(p => p.startsWith("src-tauri/runtime_packages/"));
    const runtime = join(fixture, path), original = readFileSync(runtime);
    try {
      writeFileSync(runtime, Buffer.concat([original, Buffer.from("\r\n")]));
      expect(validate()).toContain("ui-phase7b-backup-prompts-runtime-package-bytes");
    } finally { writeFileSync(runtime, original); }
    expect(validate()).toEqual([]);
    const proof = join(fixture, BACKUP_PROMPT_PROOF), proofBytes = readFileSync(proof);
    try { rmSync(proof); expect(validate().length).toBeGreaterThan(0); }
    finally { writeFileSync(proof, proofBytes); }
    const illegal = join(fixture, "src/unauthorized-backup.ts");
    try { writeFileSync(illegal, "export const bypass = true;"); expect(validate().length).toBeGreaterThan(0); }
    finally { rmSync(illegal); }
    expect(validate()).toEqual([]);
    expect(uiPhase7BWorkspaceHydrationParentReader(fixture).violations).toEqual([]);
  } finally { rmSync(fixture, { recursive: true, force: true }); }
}, 120000);
