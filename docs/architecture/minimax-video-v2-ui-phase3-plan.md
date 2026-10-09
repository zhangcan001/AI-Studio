# MiniMax Video V2 — UI Phase 3

## Baseline and scope

Planning parent: `de9ca8b9e97b22603aa9488b84de60a03c309b9f` (merged Phase 2).
UI-only shell/design-system stage; no Phase 4–7 work, new migration, package,
Recipe, Queue, Generation, import, persistence or backup changes.

## Existing dependency map

`App -> useAppRoute -> AppRoute -> one-way legacy location -> existing features`.
`App -> ShellHost -> AppShellV3 -> real Create / Runs / Library / Settings`.
App already owns bootstrap Comfy status and its runtime monitor. Project overview
reads backend nextAction/progress/readiness; the frontend does not rank actions.
Navigation/project/back continue through App's explicit draft confirmation.

## Findings and design

- Four primary entries omitted Settings; kind-only highlighting confused routes.
- Ctrl+K and overview ordinary creation actions still targeted image.
- Create exposed image-generation controls despite retired backend admission.
- Shell used scattered color values, an unbounded page scroll and limited width.
- Evolve studioTokens (including ux aliases): navy-black surfaces, accessible blue
  accent, Segoe UI/YaHei, 8px rhythm, semantic states, visible focus, reduced motion.
- Five primary entries; Settings has project/advanced subnavigation. Sidebar owns
  only ephemeral collapse state. Topbar displays project selector, breadcrumbs,
  back action and cached connection/capability facts (no additional polling).
- Keep exact historical image route/resume/shot identity; present existing results
  and history, not generator/new-shot/submit/retry controls. Result selection keeps
  its existing authority and semantics. Ordinary creation routes default video.
- Split presentation chrome only; retain Studio Store, typed transport and real
  existing feature pages. No placeholder workbench or invented progress/status.

## Successor

Create `minimax-video-v2-ui-phase3.json` and its guard/tests, parented at the actual
baseline above. Declare reviewed changed/additional paths and aggregate untouched
groups. `uiV2Changed=true`; schema/backup/queue/store/package changes remain false.
Validate live UI successor before projecting baseline bytes to the existing
technical phase3 reader. Do not edit the three historical proofs or their hashes.
Retain negative probes for metadata, undeclared edits/additions and package drift.

## Verification

Target navigation/default/history/Settings/draft/runtime/ARIA and responsive tests,
then all local source gates serially with system resource checks and exact-head CI.
Use owned isolated AI_STUDIO_DATA_ROOT and WEBVIEW2_USER_DATA_FOLDER for real Windows Tauri screenshots at
1180x760, 1280x800 and 1600x900 (overview/create/library/settings/collapsed rail).
No real user database or GPU generation. Missing Native evidence is NOT_VERIFIED,
never replaced by browser mocks or static screenshots. Stop after Phase 3.

Native acceptance found legacy child shell-grid overrides and a full-height
Create child plus runtime banner pushing Generate below the default window.
Remove only cross-shell overrides; shell owns padding and the banner/Create
share a zero-min flex column. The inner editor scrolls while Generate stays
visible. Captured real 1180x760/1280x800/1600x900 and collapsed 1000x700 windows;
no Task, ProductionBatch or GenerationLink was created by this visual pass.

## Skill application

ui-ux-pro-max guidance applied for contrast, token-driven hierarchy, meaningful
icons, keyboard/ARIA access, scroll ownership and reduced motion. Its search.py
is absent on this machine; no dependencies installed. The existing React/Tauri
desktop stack takes precedence over mobile-only examples in that Skill.
