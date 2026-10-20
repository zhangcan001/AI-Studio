# Retired UI code cleanup

Parent: bf6c4708c1de1d28ed59608d0a48bf7443373b57.

## Reachability and scope
Followed static imports, exports, dynamic literal imports and JSX from main.tsx,
then checked actual App route rendering and symbol references. Type-only files
are not dead code merely because they are absent from the runtime graph. No
blanket legacy/TODO deletion. Existing WorkflowLabPage is a navigation-only
retirement destination; its previously supplied authoring callbacks had no caller.

Removed App's dormant workflow-to-project binding, name/recipe/default helpers,
Lab-to-Create callback, unused catalog-refresh callback, notice/state wiring and
ignored temporary-selection/event-ready props. The real task event subscription,
error reporting, scope checks and task ingestion remain. Removed old local batch
folder-import/picker helpers referenced only by tests; the retired batch shell
still opens historical Production Queue with hideCreate and explicit navigation.
It no longer includes a hidden model selector or developer component names.

Narrowed retired WorkflowLabPage props to projectId/returnTo/navigate, removed
the misleading authoring-controller type dependency, shortened repeated notices
and made the generated-media empty state neutral. Formal input/provenance checks,
explicit generated-video confirmation, action errors, missing inputs, OCC and
frozen-batch warnings remain. Historical data/restore/review and current generator
bindings are untouched. Existing imported generator choices are not removed.

## Tests and compatibility
Replaced UAT tests of a fake mocked authoring page with tests of the real retired
page: image/video/dual/no-output catalogs and absent project cannot cause config
reads/writes; exact cross-project Shot navigation still works. Obsolete pure
picker tests were replaced by no-import/no-auto-start/navigation regressions.
Existing live generator/OCC/input/media tests remain, including negative cases.
Static cleanup tests start RED and prevent reintroducing authoring callbacks,
unused picker/export implementations, hidden selectors and technical copy.

## Deferred, not silently deleted
Unreachable GenerationStudio, Lab/Benchmark test harnesses, older entity-combined
panel and compatibility DTOs require a dedicated file-deletion/inheritance pass
before physical deletion. Existing historical checkpoints require their original
files; no empty placeholder files, deleted negative guards or allowlist bypasses
were used here. This bounded pass removes confirmed dead code from retained
modules without introducing a new deletion proof protocol or changing business.

## Proof
A strict successor authenticates the declared new live bytes and only then
projects original parent bytes to historical readers. Frozen proof JSON, Runtime
raw bytes, database migrations, Backup v21, Rust and queue/store authority stay
unchanged. No new dependencies or replacement implementation were needed;
local navigation-only route and existing queue APIs are reused.
