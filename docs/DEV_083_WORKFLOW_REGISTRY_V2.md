# DEV-083 Workflow Registry V2

## CURRENT_MODEL

The application currently stores a logical workflow in `workflows`, immutable
graph snapshots in `workflow_versions`, and immutable recipes in `recipes`,
but the application surface is still package-first:

- `WorkflowLibraryService::sync()` scans filesystem packages, validates them,
  registers them, and selects the highest semver version as
  `workflows.current_version_id`.
- `WorkflowRecognitionService` compares imported files against filesystem
  package manifests and therefore uses package ingestion as an identity source.
- `WorkflowOnboardingService::infer_auto_onboarding()` independently walks the
  graph and `auto_confirm()` can publish a package as part of the automatic
  import/recognition flow.
- Runtime package identity is inferred from
  `workflow_versions.package_name`; multiple recipes for one workflow version
  therefore have no authoritative package mapping.
- `WorkflowLifecycleService` owns version-oriented listing, deletion, restore,
  runtime inspection, and package scanning. `ProjectWorkflowBindingService`
  separately owns project bindings and rejects non-current versions.
- The generation catalog queries current versions directly, while the
  workspace and recognition paths read different package/version projections.
- The frontend renders one row per runtime version/package and exposes version
  deletion as a normal workflow action.

### CURRENT_ARCHITECTURE_PROBLEMS_CONFIRMED=YES

The audit confirmed all requested problems:

1. Recognition and `infer_auto_onboarding()` are two inference engines.
2. `auto_onboard`/`auto_confirm` may publish as a side effect of recognition.
3. New workflow IDs are filename slugs (`wfl_<filename>`), causing collisions.
4. Library sync changes `current_version_id` to the highest semver.
5. Project binding availability requires `version.is_current`.
6. `workflow_versions.package_name` cannot represent multiple exact recipe
   runtime artifacts.
7. Builtin/product detection is still inferred from package names in readers.
8. Deletion is primarily stored on version runtime state although the user
   operates on a logical workflow.
9. Workspace, catalog, recognition, and package scanning use separate read
   models.
10. Workflow name/category/mode update semantics are mixed into package and
    version registration.

The frozen production contracts remain outside this migration: the compiler,
graph primitives, `workflowVersionId + recipeId` history identity, DEV-078
exact admission, and the explicit ComfyUI execution/queue rules.

## TARGET_MODEL

The registry becomes the single application authority for logical workflow
CRUD, identity resolution, availability, current-version selection, remove,
restore, and purge.

### Logical entities

- `Workflow`: `id` (`wfl_<uuid>`), `name`, `sourceKind` (`PRODUCT`/`USER`),
  `libraryState` (`ACTIVE`/`REMOVED`), `currentVersionId`, timestamps.
- `WorkflowVersion`: immutable graph snapshot with its existing stable ID,
  semver, JSON, and raw SHA-256.
- `RecipeVersion`: immutable production interface with its existing stable ID,
  version, YAML, and SHA-256.
- `WorkflowRuntimeArtifact`: exact `(workflowVersionId, recipeId)` to
  `packageName` mapping with source metadata and both content hashes.

`workflow_versions.package_name` and `package_source_path` remain for
compatibility only and are never authoritative for exact runtime selection.

### Unified analysis

`WorkflowAnalysisService` is the one pure graph analysis engine. It returns the
identity hashes, inferred metadata, inputs, bindings, outputs, confidence,
issues, recipe freshness, capability enrichment, and suggested actions needed
by import, duplicate detection, re-identification, and recipe generation.
Analysis has zero database, package-store, registry, runtime-state, or project
binding writes. ComfyUI is optional capability enrichment only.

### Registry/read model

`WorkflowRegistryView` returns one row per logical workflow and nests
`versions[]` and `recipes[]`. The default catalog contains only active logical
workflows, their current version, enabled/non-archived recipes, and exact
artifact availability. Historical bindings may continue to use a non-current
version.

### User semantics

- Import is `analyze` then explicit `commit_import`.
- Exact active matches open/reuse the existing workflow; exact removed matches
  offer restore.
- Structural variants only offer “new workflow” or “new version”; there is no
  automatic merge.
- Rename changes only `workflows.name`.
- Graph changes create a new immutable workflow version; recipe changes create a
  new immutable recipe version. Existing project bindings never auto-follow.
- Normal delete removes the logical workflow (`libraryState=REMOVED`), clears
  all of its project bindings transactionally, and retains all historical
  versions, recipes, artifacts, tasks, batches, benchmarks, and shot history.
- Restore activates the logical workflow without restoring bindings; capability
  is rechecked and an unready workflow remains disabled/needs attention.
- Purge is advanced-only: USER and fully unreferenced only. PRODUCT is never
  purged.

## MIGRATION_PLAN

1. Add migration 028 (`MIGRATION_BEFORE=027`, `MIGRATION_AFTER=028`) with
   workflow source/state columns and `workflow_runtime_artifacts`, preserving
   legacy package columns and all existing IDs.
2. Backfill `library_state` from version archive state, backfill product/user
   source from package ingestion metadata, and register every existing package
   as an exact version/recipe/artifact tuple without changing IDs.
3. Add backup schema 17 support while retaining backup 16 import compatibility;
   round-trip source/state/artifact metadata.
4. Introduce registry and artifact repository ports/adapters. Make sync an
   artifact discovery/immutable-record registration operation only; it must not
   change an existing current version.
5. Add the V2 query/mutation commands and keep old commands as compatibility
   wrappers during cutover.
6. Route project availability through logical workflow state, exact version,
   exact recipe, enabled state, and non-legacy archive state; remove the
   `is_current` requirement while preserving exact binding identity.
7. Route generation catalog, workspace, onboarding duplicate detection, DEV-078
   inspection, and production package resolution through the registry/artifact
   records.

## CUTOVER_PLAN

The cutover is a strangler migration inside DEV-083:

1. Backend registry, artifact table, migration, and compatibility adapters.
2. V2 analysis/query/mutation command surface; old commands remain wrappers.
3. Frontend import flow becomes pure analyze followed by explicit add; list is
   one row per logical workflow with nested version/recipe details.
4. Production and project binding paths consume exact artifact/availability
   resolution while preserving frozen batch/task identity and DEV-078.
5. Reduce lifecycle service to runtime diagnostics/capability compatibility
   behavior.
6. Remove only proven duplicate/dead paths after tests cover identity,
   immutability, delete/restore/purge, backup/migration, project stability,
   artifact resolution, production package, and DEV-078 regressions.

No migration changes `ProductionBatch`, `ProductionBatchItem`, `Task`,
`Benchmark`, or `ShotStageConfig` workflow-version/recipe references. Normal
user deletion never hard-deletes. `AUTO_START_ON_CREATE=NO`,
`IMPLICIT_AUTO_NEXT=NO`, `EXPLICIT_USER_ARMED_NEXT=YES`, `AUTO_RETRY=NO`,
`MAX_CONCURRENT_BATCH=1`, `SECOND_QUEUE=NO`, and `SECOND_EXECUTOR=NO` remain
unchanged.
