# MiniMax Video V2 UI Phase 4 — video workbench

Planning parent: `75a7bcdc0f47b8becafa4eab5e433c87b17c1ff8` (merged UI Phase3 CI repair).

## Presentation and ownership

- Recompose the existing Create page into creative inputs, a dominant video preview/candidate strip, a run inspector and a non-scrolling generation footer. Use existing navy/blue Studio tokens; compact windows can collapse the inspector without hiding generation.
- GeneratorOption's formal mode/schema supplies mode identity, field keys and capabilities. Never infer workflow/Recipe identity from names, order or images. FIRST_LAST stays two independent slots; REF2VA keeps three ordered media lists.
- CreateController and Studio Store retain draft/submission authority. Persistent media uses the existing Recipe-scoped hook and backend receipts/OCC. No queue, task, schema, backup, IPC or runtime package changes.
- Candidate preview is local presentation state, reset on project/Shot/Recipe changes. Only the explicit select-result action changes backend selection; selection is not review. Historical image pages remain read-only generation compatibility.
- Read media only through existing project-scoped product APIs. One controlled video player, no candidate autoplay. Own and revoke image/thumbnail ObjectURLs; detach media when the owner changes. Missing media must fail honestly.
- Existing submit-time readiness, idempotency/admission, frozen RunRef retry, precise Runs navigation and Settings/Library return continue unchanged in authority. Guard late asynchronous completions by current owner/mount.

## Proof and verification

Add a separately named Phase4 live-byte checkpoint after the CI repair. Validate its declared edits/additions, untouched groups, literal Runtime Package bytes and frozen configs before projecting approved parent bytes. Propagate additions and current hashes through the prior repair adapter without rewriting any historical manifest. Keep the strict immutable Git cache and fresh live identity implementation unchanged.

Targeted frontend/media ownership and successor negative probes precede the seven serial full gates. Real isolated Windows Tauri acceptance covers 1180×760, 1280×800, 1600×900 and compact 1000×700. Fixtures are explicitly labeled; no fake runtime/progress or GPU success. Exact PR-head CI is required. Do not merge automatically and do not implement Phase5.

## Native evidence and open acceptance limitation

Real Windows Tauri/WebView inspection used isolated application data and WebView storage, the unchanged debug backend, and current Vite source. CPU-generated two-second test-pattern candidates are explicitly labeled `NATIVE FIXTURE / NO GPU`; they establish actual playback and selection behavior, not generation success. I2V and FIRST_LAST inputs were saved through official UI/IPC; database inspection confirmed independent Recipe bindings, distinct first/last assets, no additional Task, and clean foreign keys. Four required client sizes were captured; compact inspector collapse keeps the footer reachable. StrictMode replay initially detached `src` and was repaired with a dedicated regression.

Native mixed-media import is not accepted as complete: an official four-file image/image/audio/video selection committed two external image receipts, but remained pending without the final UI receipt or audio/video publication. Narrow inspection found no probe process or published audio/video file; the blocking point and cause are unproven. Reload restored UI responsiveness and permitted official image input saves. No backend, admission policy, or persistence semantics were changed to conceal this gap. The PR remains non-merge-ready until this real import behavior and remaining native mixed-reference flows are diagnosed and verified. Automated mocks and passing Rust regressions cannot replace that evidence.
