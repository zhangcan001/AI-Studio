# DEV-079 Universal Workflow Add

DEV-079 将工作流页面的普通入口收敛为 `+ 添加工作流`。用户选择 JSON 后，现有 onboarding service 在导入前先识别 API/UI/UNKNOWN/INVALID_JSON；确定性 API 工作流继续走既有自动推断、校验、发布链路，发布后刷新工作流工作区和 Generation Catalog。无法安全转换的 ComfyUI UI JSON 不会被假装成 API 工作流，而是给出可执行的 API Format 导出指引。

## Frozen verification markers

```text
DEV079_START_SHA=8a402819071860cc76d0d020d4086e8fce14ab96
BRANCH=master
ORIGIN_MASTER_START=8a402819071860cc76d0d020d4086e8fce14ab96
WORKTREE_START=clean

PRIMARY_WORKFLOW_ENTRY=ADD_WORKFLOW
API_JSON_SUPPORTED=YES
UI_JSON_DETECTED=YES
UI_JSON_SAFE_CONVERSION=NO
UI_JSON_DETECTION_AND_GUIDANCE=YES
INVALID_JSON_HANDLED=YES
UNKNOWN_JSON_HANDLED=YES
AUTO_ONBOARD_REUSED=YES
ADVANCED_EDITOR_PRESERVED=YES
NORMAL_USER_RECIPE_TERMS=NO
NORMAL_USER_BINDING_TERMS=NO
NORMAL_USER_MANIFEST_TERMS=NO
CATALOG_VISIBLE_AFTER_PUBLISH=YES
PROJECT_IMAGE_SELECTION_AFTER_ADD=YES
PROJECT_VIDEO_SELECTION_AFTER_ADD=YES
STRICT_H3_RULES_PRESERVED=YES

MIGRATION_BEFORE=027
MIGRATION_AFTER=027
BACKUP_BEFORE=16
BACKUP_AFTER=16

DEV079_CODE_SHA=2d89fc9fe41fb630c418d657173fff9281f57379
DEV079_DOC_SHA=RECORDED_IN_GIT_HISTORY
```

## Multi-agent execution

```text
MULTI_AGENT_EXECUTION=YES
MAIN=baseline, coordination, cross-layer wiring, regression, docs, Git
AGENT-A=backend format detection and existing onboarding path
AGENT-B=frontend add-workflow UX and result/error cards
AGENT-C=backend integration and React UAT coverage
CONFLICTS=ONE_SHARED_WORKTREE_OVERLAP_ON_UAT_TEST_FILE
RESOLUTION=MAIN_REVIEWED_THE_FINAL_SINGLE_TEST_FILE_AND_RERAN_ALL_TARGETED_TESTS
```

No sub-agent committed or pushed. The final code commit was created and pushed by MAIN.

## User flow

- `WorkflowWorkspace` exposes `+ 添加工作流` as the primary action.
- Manual configuration, backup import, and “检查全部兼容性” remain under `更多`; the seven-step editor is still available through the advanced fallback.
- A picker cancel leaves the previous onboarding view and catalog unchanged; replacing an import discards the previous draft through the existing lifecycle.
- A concurrent primary/advanced import is ignored while the existing onboarding operation is busy.
- A successful deterministic API import shows `✓ 工作流已添加`, name, type, purpose, version, inputs, outputs, and capability status. With a project it exposes `用于当前项目`; it also exposes `打开生成页面` and `返回工作流列表`.

Normal UI does not require the user to understand Recipe, WorkflowVersion, Binding, Manifest, or Input/Output Mapping. Existing technical details remain available in the advanced editor.

## Format support

The native onboarding service now classifies the input before API parsing:

- `API`: a non-empty top-level object whose keys are numeric node IDs and whose node values are objects with string `class_type` and object `inputs`.
- `UI`: a ComfyUI graph object containing array-valued `nodes` and `links`. The test fixture also contains `last_node_id`, `last_link_id`, `groups`, `config`, `extra`, and `version`.
- `UNKNOWN`: valid JSON that is neither of the above, including `{}` and `[]`.
- `INVALID_JSON`: JSON parsing failure.

UI-to-API conversion is deliberately not implemented in V1 because this repository has no existing deterministic converter that can safely resolve every widget, node definition, link, and output. UI JSON is therefore returned as `UNSUPPORTED_UI_FORMAT`, never published, and the UI says:

```text
检测到 ComfyUI 普通工作流 JSON。
这个格式包含界面布局信息，暂时无法可靠转换成可执行 API 工作流。
请在 ComfyUI 中将该工作流导出为 API Format JSON，然后重新选择该文件。
```

This is the documented V1 boundary, not a false conversion claim:

```text
UI_JSON_SAFE_CONVERSION=NO
UI_JSON_DETECTION_AND_GUIDANCE=YES
P2=UI_FORMAT_CONVERSION_NOT_FULLY_SUPPORTED
```

Malformed and unknown JSON receive separate ordinary-user messages. Neither path creates a package or silently changes the Catalog.

## Deterministic onboarding behavior

The existing chain remains authoritative:

```text
autoOnboardWorkflow()
  -> existing workflow onboarding service
  -> auto_onboard_bytes()
  -> existing auto_confirm()
  -> existing publish/package/catalog lifecycle
```

For a valid API graph, existing local inference is reused for workflow name, category/mode, prompt, negative prompt, seed, steps, CFG, width, height, image/video/audio inputs, and output mappings. The integration coverage proves positive and negative prompts stay distinct, numeric/media fields retain their semantics, and a deterministic image graph can reach `AUTO_PUBLISHED` without a real GPU submission.

Ambiguous input/output candidates remain `NEEDS_CONFIRMATION` and are presented through the existing `WorkflowAutoIssueView` / `WorkflowAutoIssueCandidateView` model. No candidate is selected by position or silently guessed. Missing nodes remain visible as capability diagnostics and do not trigger node installation or cloud/LLM assistance.

Generic video is classified as `category=video` using the existing generic video contract. It is not automatically labeled as `FL2VA_*` or `REF2VA_*`; strict H3 mode resolution and the no-generic-fallback rule remain unchanged.

## Catalog and project workflow closure

After a real package publication, `WorkflowWorkspace` refreshes its production workspace and calls the existing `onCatalogChanged()` callback. The DEV-079 integration test then re-reads the existing Generation Catalog and verifies the published `workflowVersionId + recipeId` pair.

`ProjectWorkflowSettings` was not reimplemented. Its existing catalog filtering continues to expose published image recipes in the image default candidate and published video recipes in the video default candidate. The project workflow UAT regression covers both candidate paths and strict H3 mode selection.

## Tests and verification

```text
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check                         PASS
cargo check --manifest-path src-tauri/Cargo.toml                                  PASS
cargo test --manifest-path src-tauri/Cargo.toml --lib workflow_onboarding -- --test-threads=1
  22 passed, 0 failed                                                             PASS
cargo test --manifest-path src-tauri/Cargo.toml --test dev079_workflow_add -- --test-threads=1
  6 passed, 0 failed                                                              PASS
cargo test --manifest-path src-tauri/Cargo.toml workflow_lifecycle -- --test-threads=1
  7 passed, 0 failed                                                              PASS
cargo test --manifest-path src-tauri/Cargo.toml generation_catalog -- --test-threads=1
  1 DEV-079 integration assertion passed, 0 failed                                PASS
cargo test --manifest-path src-tauri/Cargo.toml --test dev078_production_start_admission -- --test-threads=1
  3 passed, 0 failed                                                              PASS
cargo test --manifest-path src-tauri/Cargo.toml --quiet
  729 lib tests passed, 0 failed, 1 ignored; all integration targets passed        PASS

pnpm exec vitest run src/features/workflows/WorkflowAddUat.test.tsx src/features/runtime/pack05.test.ts src/features/workflows/WorkflowWorkspace.test.ts --reporter=dot
  3 files, 16 tests passed                                                        PASS
pnpm test -- WorkflowWorkspace WorkflowAddUat pack05
  4 files, 18 tests passed                                                        PASS
pnpm test -- ProjectWorkflowSettings projectWorkflowResolution ProjectWorkflowPreflight ProjectProductionReadiness ProjectProductionReadinessUat
  7 files, 56 tests passed                                                        PASS
pnpm test
  110 files, 514 tests passed                                                     PASS
pnpm exec tsc --noEmit                                                            PASS
pnpm build                                                                        PASS
git diff --check                                                                  PASS
pnpm tauri build                                                                  PASS
```

The first full Rust run had one existing timing-sensitive cancellation test fail (`task did not reach SUCCEEDED`). That test passed in isolation, and the immediately repeated full Rust run passed with `729 passed, 0 failed, 1 ignored`. No DEV-079 file is involved in that test.

The Tauri build produced:

```text
src-tauri/target/release/ai-studio.exe
src-tauri/target/release/bundle/msi/AI Studio_1.0.0_x64_en-US.msi
src-tauri/target/release/bundle/nsis/AI Studio_1.0.0_x64-setup.exe
```

No real MiniMax H3 generation or GPU submission was run. DEV-079 integration tests use deterministic local adapters and fixture databases.

## Git and scope audit

Code and tests were committed and pushed first:

```text
2d89fc9fe41fb630c418d657173fff9281f57379 feat(workflows): add streamlined workflow onboarding
```

The documentation is committed separately with the required message:

```text
docs: record DEV-079 verification
```

There is no new migration, table, workflow model, executor, runtime system, queue behavior, batch strategy, automatic node installation, cloud workflow sync, or LLM integration. The migration set remains 027 and the backup count remains 16.

## DEV-079 P1 fix closeout

The P1 fix keeps the existing project workspace and project workflow settings as the single configuration surface. The two result actions now have independent callbacks:

```text
打开生成页面 -> onOpenStudio -> openPublishedWorkflow
用于当前项目 -> onUseInProject -> refresh Catalog -> exact workflowId + recipeId check -> projects workspace
```

`用于当前项目` never auto-binds a project configuration. It re-reads the Catalog, rejects a missing current project or missing exact recipe pair without navigation or database mutation, and opens the current project's existing `ProjectWorkflowSettings`. The user selects and explicitly saves the exact `workflowVersionId + recipeId` there. It does not fall back to another recipe with the same `workflowId`, and it does not fall back to the Studio page.

Ambiguous import issue selection now uses a unique key composed of `code + field-or-NONE + issueIndex`. The key is used for selected state, radio groups, and rendered issue identity. Selection is cleared when the draft, state, or issue fingerprint changes. Two same-code issues therefore remain independently selectable and each resolve callback receives its own exact issue and candidate.

The React closure UAT is intentionally two-stage: the real `WorkflowWorkspace` crosses a mocked Tauri boundary and refreshes the Catalog; the independent project callback then renders the real `ProjectWorkflowSettings`, verifies both image/video exact candidates, proves no write before Save, and verifies the exact binding payload after Save.

```text
DEV079_P1_START_SHA=2c21a1ff9767541227d9b260037514290e8900a1
P1_MULTI_AGENT_EXECUTION=YES
P1_MAIN=workspace wiring, integration, verification, docs, Git
P1_AGENT_A=App project-destination callback and exact Catalog guard
P1_AGENT_B=ambiguity selection key and issue isolation regression
P1_AGENT_C=two-stage React closure UAT with real Workspace and ProjectWorkflowSettings
P1_CONFLICTS=NONE
P1_SUBAGENTS_COMMITTED_OR_PUSHED=NO

DEV079_P1_01_USE_IN_PROJECT_NAVIGATION=FIXED
DEV079_P1_02_AMBIGUITY_SELECTION_COLLISION=FIXED
DEV079_P2_01_PROJECT_CLOSURE_REACT_UAT=FIXED
USE_IN_PROJECT_OPENS_PROJECT_WORKFLOW_SETTINGS=YES
USE_IN_PROJECT_AUTO_BINDS=NO
USE_IN_PROJECT_OPEN_SETTINGS=YES
USE_IN_PROJECT_AUTO_BIND=NO
USE_IN_PROJECT_FALLBACK_TO_OPEN_STUDIO=NO
EXACT_PROJECT_RECIPE_CHECK=YES
AMBIGUITY_SELECTION_KEY=CODE_PLUS_FIELD_PLUS_INDEX
MULTIPLE_SAME_CODE_ISSUES_ISOLATED=YES
ADD_TO_PROJECT_SINGLE_REACT_CLOSURE_UAT=NO
TWO_STAGE_REACT_CLOSURE_UAT=YES

P1_DIRECT_FRONTEND_TARGET=4_FILES_11_TESTS_PASS
P1_REQUESTED_FRONTEND_TARGET=5_FILES_18_TESTS_PASS
P1_PROJECT_WORKFLOW_REGRESSION=7_FILES_56_TESTS_PASS
P1_FULL_FRONTEND=112_FILES_519_TESTS_PASS
P1_DEV079_RUST=6_TESTS_PASS
P1_WORKFLOW_ONBOARDING=22_TESTS_PASS
P1_DEV078_RUST=3_TESTS_PASS
P1_FULL_RUST=729_PASS_0_FAIL_1_IGNORED
P1_TYPESCRIPT=PASS
P1_FRONTEND_BUILD=PASS
P1_TAURI_BUILD=PASS
P1_RUST_FMT=PASS
P1_DIFF_CHECK=PASS

MIGRATION_P1_BEFORE=027
MIGRATION_P1_AFTER=027
BACKUP_P1_BEFORE=16
BACKUP_P1_AFTER=16
ARCHITECTURE_UNCHANGED=YES
UI_JSON_CONVERSION_REMAINS_P2=YES
DEV079_P1_FIX_SHA=RECORDED_IN_GIT_HISTORY
```

## DEV-079 Navigation Fix closeout

The primary Studio rail now exposes `工作流` as an independent first-level entry between `审核` and `分析`. Its existing `workflows` workspace route is reused, so navigation renders the existing `WorkflowWorkspace` and its `+ 添加工作流` entry without changing onboarding, persistence, queue, generation, project binding, or ComfyUI runtime behavior.

```text
DEV079_NAVIGATION_WORKFLOW_ENTRY=FIXED
GLOBAL_RAIL_WORKFLOW_ITEM=YES
GLOBAL_RAIL_WORKFLOW_DESTINATION=workflows
GLOBAL_RAIL_WORKFLOW_ICON=WORKFLOW_NODE_STROKE_24X24
GLOBAL_RAIL_WORKFLOW_ORDER=PROJECT_CREATION_ASSETS_PRODUCTION_REVIEW_WORKFLOWS_ANALYSIS_SETTINGS
WORKFLOW_ACTIVE_RAIL_ITEM=workflows
WORKFLOW_PRIMARY_ADD_ENTRY_VISIBLE=YES
WORKFLOW_MENU_LEVEL=PRIMARY
NAVIGATION_TARGETED_FRONTEND=8_FILES_28_TESTS_PASS
NAVIGATION_FULL_FRONTEND=113_FILES_523_TESTS_PASS
NAVIGATION_TYPESCRIPT=PASS
NAVIGATION_FRONTEND_BUILD=PASS
NAVIGATION_CARGO_CHECK=PASS
NAVIGATION_TAURI_BUILD=PASS
NAVIGATION_DIFF_CHECK=PASS

MIGRATION_BEFORE=027
MIGRATION_AFTER=027
BACKUP_BEFORE=16
BACKUP_AFTER=16
NEW_DB_TABLE=NO
NEW_TAURI_COMMAND=NO
NEW_WORKSPACE_TYPE=NO
NEW_ROUTE_SYSTEM=NO
DEV080_NOT_STARTED=YES
```

## DEV-079 Persistent Project Use closeout

The workflow list now keeps `用于当前项目` on every non-archived published workflow row. The row resolves the newest recipe that is actually present in the current Generation Catalog by exact `workflowVersionId + recipeId` identity, while reusing the existing `onUseInProject` callback. The action opens the existing project workflow settings and never writes project bindings automatically.

```text
WORKFLOW_LIST_PERSISTENT_USE_IN_PROJECT=YES
USE_IN_PROJECT_IMPORT_RESULT=YES
USE_IN_PROJECT_WORKFLOW_ROW=YES
NO_ACTIVE_PROJECT_BUTTON_HIDDEN=NO
NO_ACTIVE_PROJECT_BUTTON_VISIBLE_DISABLED=YES
WORKFLOW_ROW_EXACT_RECIPE_MATCH=WORKFLOW_VERSION_ID_PLUS_RECIPE_ID
WORKFLOW_ROW_USE_IN_PROJECT_AUTO_BIND=NO
WORKFLOW_ROW_ARCHIVED_ACTION=HIDDEN
WORKFLOW_ROW_CATALOG_MISSING_ACTION=VISIBLE_DISABLED
WORKFLOW_ROW_RECIPE_SELECTION=LATEST_CATALOG_MATCH_BY_REVERSE_SCAN
PERSISTENT_PROJECT_USE_TARGETED=8_FILES_34_TESTS_PASS
PERSISTENT_PROJECT_USE_FULL_FRONTEND=114_FILES_530_TESTS_PASS
PERSISTENT_PROJECT_USE_TYPESCRIPT=PASS
PERSISTENT_PROJECT_USE_BUILD=PASS
PERSISTENT_PROJECT_USE_CARGO_CHECK=PASS
PERSISTENT_PROJECT_USE_TAURI_BUILD=PASS_ISOLATED_TARGET
PERSISTENT_PROJECT_USE_DIFF_CHECK=PASS
MIGRATION_BEFORE=027
MIGRATION_AFTER=027
BACKUP_BEFORE=16
BACKUP_AFTER=16
NEW_DB_TABLE=NO
NEW_TAURI_COMMAND=NO
PROJECT_BINDING_SCHEMA_CHANGED=NO
PRODUCTION_QUEUE_CHANGED=NO
```

Regression coverage keeps the import-result `用于当前项目` action, existing project workflow settings, and the primary workflow navigation entry intact. The isolated Tauri release build produced both MSI and NSIS bundles under `D:\AI-Studio-DEV079-20260903-tauri-target`; the canonical release executable was left running and was not interrupted.

## Final gate

```text
P0=NONE
P1=NONE
P2=UI_FORMAT_CONVERSION_NOT_FULLY_SUPPORTED
P3=NONE
DEV079_UNIVERSAL_WORKFLOW_ADD=PASS
DEV079_P1_PROJECT_WORKFLOW_ADD_CLOSURE=PASS
DEV079_NAVIGATION_WORKFLOW_ENTRY=PASS
DEV079_PERSISTENT_PROJECT_USE=PASS
DEV-080_NOT_STARTED=YES
```

DEV-079 is complete. Do not automatically start DEV-080.
