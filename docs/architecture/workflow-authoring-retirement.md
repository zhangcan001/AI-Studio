# Workflow authoring UI retirement

Parent: 448f0f079f3234084163225d0e16eed70dbba836.

## Product decision
Remove workflow authoring/import/Benchmark navigation from Settings, project generators and Create. Future workflows are shipped as reviewed embedded packages with explicit Recipe mappings and the existing immutable identity/qualification checks. No generic automatic-import replacement is added.

Legacy `advanced-workflows` locators remain parseable for resume/Back compatibility, but render only a no-side-effect retirement notice with project generator/return actions. The notice imports the old workspace props as a TypeScript type only: it does not load the controller, onboarding, editor, Benchmark or repair panes. Self-referential retired return locators are suppressed. Ordinary environment/Task diagnostics remain available.

## Compatibility and scope
Existing exact WorkflowVersion+Recipe bindings (including prior imported generators), historical inputs/results, Task/RunRef, queue execution, backups and project isolation are not changed. No database cleanup, migration, model download, Runtime Package edit, cloud call or GPU submission. Legacy authoring implementation and backend IPC remain as compatibility/internal code; this is not an IPC access-control redesign. Older adapter tests continue protecting binding/OCC behavior without constituting a live UI entry.

Phase7 retirement matrix is historical and unchanged. Its current UI regression checks are updated to require the replacement notice and preserved implementation files rather than the obsolete accessible editor. A strict successor validates every legal changed/added file and projects exact parent bytes into the historical reader; old manifests and their hashes are never repinned.

## Reuse
Inspected official ComfyUI packaged template approach: https://github.com/Comfy-Org/workflow_templates and https://github.com/Comfy-Org/docs/blob/main/interface/features/template.mdx. Existing local builtin registry and typed generator settings already implement the required mechanism, so no upstream code/dependency was copied. UI/UX skill applied: retain current design tokens/stack, clear retirement copy, explicit escape routes, no dead import CTA. Its optional local search.py is missing; no installation attempted.

## Guard metadata compatibility fix
Full Boundary validation found WORKBENCH_VISUAL discarded successor.afterHashes while forwarding validated successor paths/bytes. Newly legal App source edits therefore passed the strict proof chain but the legacy BackendBoundary live-file pin comparison still used an older hash. Merge only hashes from that already validated successor, preserving all scope/proof/raw-byte checks and frozen JSON. A regression now verifies the top-level historical reader's App afterHash against live bytes after successful validation. This is not a repin of historical manifests.
A second legacy seam mixed latest validated afterHashes with ciBudgetSuccessor.read(path), which intentionally returns historical parent bytes. After repairing metadata propagation, the StyleBoundary correctly exposed the mismatch. Compare the authenticated current digest to live canonical bytes, not a historical projection; retain historical readers for their original proof checks. Wrong ownership, undeclared edits, altered proofs and Runtime raw-byte probes remain required. No historical JSON/hash or CSS budget is edited.
