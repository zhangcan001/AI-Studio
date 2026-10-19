// @vitest-environment node
import { beforeAll, it, expect } from "vitest";
import { readFileSync, writeFileSync, copyFileSync, mkdtempSync, mkdirSync, rmSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { trackTaskUpdateRpc } from "./boundary-rpc-yield.mjs";
import { R1_METADATA_BASE, R1_METADATA_PROOF, uiPhase7BR1MetadataParentReader } from "../ui-phase7b-r1-metadata-successor-guard.mjs";
import { uiPhase7BWorkspaceHydrationParentReader, HYDRATION_PROOF } from "../ui-phase7b-workspace-hydration-successor-guard.mjs";

// Validate the cold immutable proof chain once before timed negative probes.
// Each mutation still calls the reader, including its live-byte freshness check;
// the five-probe test retains its original 5s timeout and every rejection check.
beforeAll(() => {
  expect(uiPhase7BR1MetadataParentReader(".").violations).toEqual([]);
}, 120000);

it("awaits genuine task-update acknowledgements and retains RPC rejections", async () => {
  let acknowledge;
  const response = new Promise(resolve => { acknowledge = resolve; });
  const args = [[], []];
  const original = { onTaskUpdate(...received) { expect(this).toBe(original); expect(received).toEqual(args); return response; }, other: () => "unchanged" };
  const tracked = trackTaskUpdateRpc(original);
  expect(tracked.rpc.other()).toBe("unchanged");
  expect(tracked.rpc.onTaskUpdate(...args)).toBe(response);
  let drained = false;
  const flush = tracked.flush().then(() => { drained = true; });
  await new Promise(resolve => setImmediate(resolve));
  expect(drained).toBe(false);
  acknowledge(); await flush; expect(drained).toBe(true);

  const error = new Error("real RPC failure");
  const denied = trackTaskUpdateRpc({ onTaskUpdate: () => Promise.reject(error) });
  await expect(denied.rpc.onTaskUpdate([], [])).rejects.toBe(error);
  await expect(denied.flush()).rejects.toBe(error);
});

it("rejects undeclared byte changes in every entity-reference preparation seam", () => {
  for (const path of [
    "src-tauri/src/application/production_preparation_service.rs",
    "src-tauri/src/application/shot_batch_service.rs",
    "src-tauri/src/application/shot_service/creation.rs",
    "src-tauri/src/application/shot_video_input_service.rs",
    "src/product/errors.ts",
  ]) {
    expect(uiPhase7BR1MetadataParentReader(".", {
      [path]: readFileSync(path, "utf8") + "\n// unapproved entity-reference mutation\n",
    }).violations).toContain("ui-phase7b-r1-metadata-changed:" + path);
  }
});

it("validates CI workflow bytes before projecting the frozen workflow for historical readers", () => {
  const workflow = ".github/workflows/ci.yml";
  const proof = JSON.parse(readFileSync(R1_METADATA_PROOF, "utf8"));
  expect(proof.files[workflow]).toBeDefined();
  const view = uiPhase7BR1MetadataParentReader(".");
  expect(view.violations).toEqual([]);
  expect(view.read(workflow)).toEqual(execFileSync("git", ["show", R1_METADATA_BASE + ":" + workflow], { encoding: "utf8" }));
  expect(uiPhase7BR1MetadataParentReader(".", {
    [workflow]: readFileSync(workflow, "utf8") + "\n# unapproved workflow change\n",
  }).violations).toContain("ui-phase7b-r1-metadata-changed:" + workflow);
  expect(uiPhase7BR1MetadataParentReader(".", {
    [R1_METADATA_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files,
      [workflow]: { ...proof.files[workflow], afterHash: "0".repeat(64) },
    } }),
  }).violations).toContain("ui-phase7b-r1-metadata-changed:" + workflow);
}, 120000);

it("keeps full Rust checks with a bounded cold budget and master-push-only cache writes", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const rust = workflow.split("  rust:\n")[1].split("  frontend_tests:\n")[0];
  const budget = Number(/timeout-minutes: (\d+)/.exec(rust)?.[1]);
  expect(budget).toBeGreaterThanOrEqual(60);
  expect(budget).toBeLessThanOrEqual(75);
  expect(rust).toContain("save-if: ${{ github.event_name == 'push' && github.ref == 'refs/heads/master' }}");
  expect(rust).toContain('cache-targets: "false"');
  expect(rust).toContain('AI_STUDIO_LIVE_COMFY: "0"');
  expect(rust).toContain("cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check");
  expect(rust).toContain("cargo check --manifest-path src-tauri/Cargo.toml --all-targets");
  expect(rust).toContain("cargo test --manifest-path src-tauri/Cargo.toml --all-targets -- --test-threads=1");
});

it("discovers all Boundary files explicitly and partitions them without omissions or duplicates", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  expect(workflow).toContain("run: pnpm test:boundary --shard=${{ matrix.shard }}/3");
  const runner = "scripts/run-boundary-tests.mjs";
  const list = (...args) => JSON.parse(execFileSync(process.execPath, [runner, "--list", ...args], { encoding: "utf8" }));
  const expected = execFileSync("git", ["ls-files", "--cached", "--others", "--exclude-standard"], { encoding: "utf8" })
    .trim().split(/\r?\n/).filter(p => /^src\/app\/.*Boundary.*\.test\.tsx?$/.test(p)
      || /^scripts\/testing\/.*[Bb]oundary.*\.test\.tsx?$/.test(p)
      || p === "scripts/testing/immutable-git-cache.test.ts").sort();
  expect(expected).toContain("src/app/MiniMaxVideoPhase1Boundary.test.ts");
  expect(expected).toContain("src/app/WorkspaceErrorBoundary.test.tsx");
  expect(list()).toEqual(expected);
  const partitions = [1, 2, 3].map(i => list(`--shard=${i}/3`));
  expect(partitions.flat().sort()).toEqual(expected);
  expect(new Set(partitions.flat()).size).toBe(expected.length);
  const counts = partitions.map(files => files.length);
  expect(Math.max(...counts) - Math.min(...counts)).toBeLessThanOrEqual(1);
  expect(() => list("--shard=0/3")).toThrow();
  expect(() => list("--shard=1/0")).toThrow();
});

it("collects the real boundary file but never mirrored tests in an abandoned Phase4 probe", () => {
  const root = mkdtempSync(join(process.cwd(), ".codex-ui-phase4-probe-discovery-"));
  try {
    const relative = "src/app/UiPhase7ASourceReadyBoundary.test.ts";
    const mirror = join(root, relative);
    mkdirSync(dirname(mirror), { recursive: true });
    writeFileSync(mirror, "throw new Error('evidence copy must not execute');\n");
    const files = execFileSync(process.execPath,
      ["node_modules/vitest/vitest.mjs", "list", "--filesOnly", relative], { encoding: "utf8" })
      .trim().replaceAll("\\", "/").split(/\r?\n/);
    expect(files).toEqual([relative]);
  } finally { rmSync(root, { recursive: true, force: true }); }
});

it("validates the backup correction before projecting the frozen 0c3 backup checkpoint", () => {
  const view = uiPhase7BR1MetadataParentReader(".");
  expect(view.violations).toEqual([]);
  expect(uiPhase7BWorkspaceHydrationParentReader(".").violations).toEqual([]);
  const path = "src-tauri/src/infrastructure/database/repositories/project_backup.rs";
  expect(view.read(path)).toEqual(execFileSync("git", ["show", R1_METADATA_BASE + ":" + path], { encoding: "utf8" }));
}, 120000);

it("projects the explicitly scoped Studio Store and protects its new storyboard regressions", () => {
  const store = "src/stores/studioStore.ts", tests = "src/features/create/StoryboardSafety.test.tsx";
  const proof = JSON.parse(readFileSync(R1_METADATA_PROOF, "utf8"));
  const view = uiPhase7BR1MetadataParentReader(".");
  expect(view.violations).toEqual([]);
  expect(proof.files[store].beforeHash).toMatch(/^[a-f0-9]{64}$/);
  expect(proof.files[tests].beforeHash).toBeNull();
  expect(view.read(store)).toEqual(execFileSync("git", ["show", R1_METADATA_BASE + ":" + store], { encoding: "utf8" }));
  expect(uiPhase7BR1MetadataParentReader(".", { [store]: readFileSync(store, "utf8") + "\n// undeclared draft state\n" }).violations)
    .toContain("ui-phase7b-r1-metadata-changed:" + store);
  expect(uiPhase7BR1MetadataParentReader(".", { [tests]: readFileSync(tests, "utf8") + "\n// undeclared regression edit\n" }).violations)
    .toContain("ui-phase7b-r1-metadata-added:" + tests);
}, 120000);

it("projects the explicit Library polling repair and rejects modified read regression bytes", () => {
  const view = uiPhase7BR1MetadataParentReader(".");
  expect(view.violations).toEqual([]);
  for (const path of ["src/features/library/LibraryController.ts", "src/features/library/LibraryController.performance.test.tsx"]) {
    expect(view.read(path)).toEqual(execFileSync("git", ["show", R1_METADATA_BASE + ":" + path], { encoding: "utf8" }));
    expect(uiPhase7BR1MetadataParentReader(".", { [path]: readFileSync(path, "utf8") + "\n// undeclared polling edit\n" }).violations)
      .toContain("ui-phase7b-r1-metadata-changed:" + path);
  }
}, 120000);

it("rejects forged identity/hashes, protected reader edits and undeclared changes", () => {
  const proof = JSON.parse(readFileSync(R1_METADATA_PROOF, "utf8"));
  const repository = "src-tauri/src/infrastructure/database/repositories/project_backup.rs";
  const service = "src-tauri/src/application/project_backup_service.rs";
  const cases = [
    { [R1_METADATA_PROOF]: JSON.stringify({ ...proof, parentHead: "forged" }) },
    { [R1_METADATA_PROOF]: JSON.stringify({ ...proof, files: { ...proof.files, [repository]: { ...proof.files[repository], afterHash: "0".repeat(64) } } }) },
    { [R1_METADATA_PROOF]: "" },
    { [repository]: readFileSync(repository, "utf8") + "\n// undeclared SQL change\n" },
    { [service]: readFileSync(service, "utf8").replace('stage_prompt_counts.get(shot.id.as_str()).copied() != Some(2)', 'false') },
    { [HYDRATION_PROOF]: readFileSync(HYDRATION_PROOF, "utf8") + "invalid" },
    { "src-tauri/migrations/999_unauthorized.sql": "SELECT 1;" },
  ];
  for (const override of cases) expect(uiPhase7BR1MetadataParentReader(".", override).violations.length).toBeGreaterThan(0);
  expect(uiPhase7BR1MetadataParentReader(".").violations).toEqual([]);
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
    const validate = () => uiPhase7BR1MetadataParentReader(fixture).violations;
    expect(validate()).toEqual([]);
    const path = paths.find(p => p.startsWith("src-tauri/runtime_packages/"));
    const runtime = join(fixture, path), original = readFileSync(runtime);
    try {
      writeFileSync(runtime, Buffer.concat([original, Buffer.from("\r\n")]));
      expect(validate()).toContain("ui-phase7b-r1-metadata-runtime-package-bytes");
    } finally { writeFileSync(runtime, original); }
    expect(validate()).toEqual([]);
    const proof = join(fixture, R1_METADATA_PROOF), proofBytes = readFileSync(proof);
    try { rmSync(proof); expect(validate().length).toBeGreaterThan(0); }
    finally { writeFileSync(proof, proofBytes); }
    const illegal = join(fixture, "src/unauthorized-backup.ts");
    try { writeFileSync(illegal, "export const bypass = true;"); expect(validate().length).toBeGreaterThan(0); }
    finally { rmSync(illegal); }
    const workflow = join(fixture, ".github/workflows/ci.yml"), workflowBytes = readFileSync(workflow);
    try {
      writeFileSync(workflow, Buffer.concat([workflowBytes, Buffer.from("\n# changed after cache hit\n")]));
      expect(validate()).toContain("ui-phase7b-r1-metadata-changed:.github/workflows/ci.yml");
    } finally { writeFileSync(workflow, workflowBytes); }
    expect(validate()).toEqual([]);
    try { rmSync(workflow); expect(validate().length).toBeGreaterThan(0); }
    finally { writeFileSync(workflow, workflowBytes); }
    const illegalWorkflow = join(fixture, ".github/workflows/unauthorized.yml");
    try { writeFileSync(illegalWorkflow, "name: bypass\n"); expect(validate().length).toBeGreaterThan(0); }
    finally { rmSync(illegalWorkflow); }
    expect(validate()).toEqual([]);
    expect(uiPhase7BWorkspaceHydrationParentReader(fixture).violations).toEqual([]);
  } finally { rmSync(fixture, { recursive: true, force: true }); }
}, 120000);
