# M3 Creation Expansion Pack 05

Date: 2026-08-10
Development line: `0.3.0`
Release status: development only; no `v0.3.0` tag or GitHub Release created.

## Scope

Pack 05 extends the existing generic Workflow / Recipe / Schema / Capability path. It does not add model-name branches, bundle model files, or download models automatically.

| Pack item | Implementation | Gate status |
| --- | --- | --- |
| P05-01 Generic onboarding architecture | API/UI workflow quality inspection, explicit UI-format conversion guidance, existing onboarding validation chain | CODE PASS; retained for the frozen Kera2 + MiniMax H3 scope |
| P05-02 Runtime selection UX | Type and keyword filters in the creation launcher | CODE PASS |
| P05-03 Image-to-image / reference UX | Generic reference-mode detection and media-input guidance | CODE PASS |
| P05-04 Video workflow UX | Generic video-mode guidance with duration and reference-input reminder | CODE PASS |
| P05-05 Runtime parameter profiles | Backend-persisted, workflow-scoped profiles in `%LOCALAPPDATA%\AIStudio\AIStudioData\config\settings.json`; direct integer field-key binding and one-time legacy localStorage migration; no concurrency setting | CODE PASS |
| P05-06 Workflow import quality | JSON size/shape checks, API-vs-UI format detection, exact credential-key detection, path warnings, node/output quality signals | CODE PASS |
| P05-07 Production queue multi-runtime validation | Strict sequence, enabled/readiness/capability checks, duplicate and unavailable-runtime checks, optional multi-runtime requirement | CODE PASS |
| P05-08 Release-grade runtime gate | Separates code blockers from missing local runtime input and reports `PASS`, `BLOCKED`, or `ENVIRONMENT_BLOCKED` | CODE PASS; active production scope is frozen to Kera2 + MiniMax H3 |

## Local runtime boundary

The local ComfyUI endpoint is healthy and exposes installed models and capabilities. The workflow library contains the two validated production packages. Pack 05 did not guess a model name, alter existing packages, or download anything.

## Verification

The pure Pack 05 contracts cover runtime classification, parameter bounds and application, import quality, multi-runtime queue admission, and release-gate state separation. The existing Kera2 and H3 runtime paths remain unchanged.

The v0.2.0 release audit is recorded separately in `docs/POST_RELEASE_AUDIT_0.2.0.md`; its tag, Release, binaries, installer, and release commit remain frozen.

## Historical scope note

Pack 05 originally recorded an additional-runtime onboarding exploration. That exploration is historical-only and is not an active 0.3.0 product requirement. The current production runtime scope is frozen to Kera2 image keyframes and MiniMax H3 reference-image-to-video; the generic onboarding/Recipe/Capability architecture remains available without expanding that scope.
