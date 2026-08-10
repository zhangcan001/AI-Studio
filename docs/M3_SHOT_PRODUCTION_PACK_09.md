# M3 SHOT PRODUCTION PACK 09 = CODE PASS / LIVE PENDING

Date: 2026-08-10
Development line: `0.3.0`
Active production scope: Kera2 image keyframes + MiniMax H3 reference-image-to-video.

## Contract

Pack 09 adds a project-scoped Shot workspace with the flow:

`Shot → prompt/provenance snapshot → Kera2 image candidates → select keyframe → H3 reference image → H3 video candidates → select final video`

Shot execution truth is derived from persisted stage configuration, selected assets, and linked normal Task statuses. The frontend does not persist `RUNNING` or `FAILED` as Shot rows. The backend validates project ownership, media type, linked-task stage, and result-asset existence before selection.

Migration `010_shot_production.sql` adds only `shots`, `shot_stage_configs`, `shot_reference_assets`, and `shot_generation_links`. Migrations `001–009` remain immutable. No `ALTER tasks`, `shot_outputs`, ShotTask, Executor, direct `/prompt`, or new Task system was added. Shot deletion removes orchestration metadata/configuration/relations only; Task, Snapshot, Asset, and Prompt Library records remain available.

Prompt Library loading explicitly copies text and entry/version provenance. Inline edits clear provenance. Stage scalar JSON accepts only Recipe-owned integer/seed values; image/video capability is determined from Recipe outputs, not model-name branches. Video generation requires a video config and a selected current-project image; success of image generation never auto-submits H3.

## Verification

- Rust repository/domain coverage includes migration schema, CRUD, reorder, project isolation, scalar validation, Reference media type checks, selection checks, and generation-link checks.
- Backup v4 accepts v1/v2/v3/v4, restores legacy archives with empty Shot data, exports/restores Shot stage configs/References/generation links, remaps Shot/task/asset/prompt/version/batch-item IDs, validates integrity, and rolls back atomically.
- Frontend includes the Chinese Shot workspace labels: `镜头制作`, `新建镜头`, `生成关键帧`, `图片候选`, `设为关键帧`, `生成视频`, `视频候选`, `设为最终视频`, `在创作中打开`.
- The local database migration smoke completed against the existing database: 009 → 010, four Shot tables queryable, and no pre-existing Shot rows; fresh initialization covers 001 → 010.
- Full regression completed: `cargo fmt --all -- --check`, `cargo check`, `cargo test -- --test-threads=1` (`309 passed`), `pnpm test -- --reporter=dot` (`31 files / 96 passed`), `pnpm build`, `git diff --check`, and `pnpm tauri build` (MSI + NSIS bundles).

The Pack 08/09 source and automated checks are code-pass. A controllable desktop Live GPU/UI chain was not observable in this audit, and no Kera2/H3 Shot generation result is claimed without that evidence. Therefore the honest gate is `CODE PASS / LIVE PENDING`.
