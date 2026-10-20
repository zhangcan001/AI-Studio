# Generation progress background refresh

Parent: ec26f20564ecbc15fd95beb3bf60b0f56e32f5a3.
Initial tested checkout: a768ed5811964c8679f6792a9be68741c244eaf0.
Upstream ec26f20 corrects the retired WorkflowLab boundary assertion and its
checkpoint registration; retained intact before adding this successor.

## Defect and boundary
Task notifications and the 8-second fallback poll called the same foreground
workbench reader, toggling every row and header action disabled opacity.
Library notifications likewise used foreground loading despite quiet polling.
No browser reload or DOM remount was necessary to reproduce the flash.

## Smallest repair
Workbench background reads keep existing controls/projection visible. Initial
and explicit refresh retain their foreground lock; per-row mutation and creation
locks, formal readiness, OCC and Production Queue admission remain unchanged.
A non-resetting 500ms notification window bounds event-driven reads; notifications
received during an in-flight read remain pending until it finishes. The 8-second
fallback remains. Only an owning request may release its read lock. Project
changes and unmount dispose pending events and fence late results.
Library notifications reuse background refresh, retain fresh tags and the
existing 150ms debounce. Real invalidations still supersede obsolete reads;
existing epoch/scope checks prevent obsolete results from replacing fresh data. Its 5-second
fallback and user-initiated foreground reads remain unchanged.

## Reuse/provenance
Local reuse: LibraryController background flag and RunsController single-flight
pending refresh pattern. Primary reference consulted for loading/background-fetch
separation: https://github.com/TanStack/query/blob/main/docs/framework/react/guides/queries.md
No dependency or third-party implementation copied; existing React lifecycle
and typed product facade retained. No new business queue, executor or model.

## Proof/security
This successor validates live declared bytes against the exact immutable parent,
then projects validated historical bytes into prior readers. Frozen proof JSON,
Runtime Package raw bytes, Rust, schema and Backup v21 are unchanged. Negative
checks cover forged proof, live changes/deletion, undeclared additions and raw
runtime changes; restored original bytes must validate again.

## Regression evidence
Previously RED: background polling dims idle buttons; 20 event burst triggers
unbounded reads; library event shows foreground loading; final updates can be
lost during a read. Permanent tests also protect manual-refresh locking, same
button node, project read ownership and timer disposal. These are frontend
regression tests, not a claim of Native GPU acceptance. No GPU task submitted.
