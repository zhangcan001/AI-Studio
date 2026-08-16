# M3 Live Validation 0.3.0

## DEV-016C — MiniMax H3 REF2VA 多参考资产实机验证

- 日期：2026-08-16
- Git HEAD / App build commit：`d8dabc9a104f7b14cfd041cbc62c5cfde53678ac`
- 分支：`master`
- 工作树：验证开始时干净；本次仅新增本证据文档
- ComfyUI：在线，`0.33.0`
- GPU：NVIDIA GeForce RTX 5060 Ti，CUDA runtime `13.0`
- 运行方式：MiniMax H3 REF2VA FAST，本轮严格实机任务单项串行
- 目标参数：5 秒、864×480、固定 seed

### Runtime package

- Workflow ID：`wfl_minimax_h3_reference_video`
- Workflow version ID：`wfv_972fcf31-feeb-483b-ab4a-7c7847256c10`
- Workflow version：`1.3.0`
- Workflow SHA-256：`817d0296122c275694d3adc5541e8df1b41af470c0e4e9f4a12bdb9f3962539d`
- Recipe ID：`rcp_e3ec86e3-9757-44f3-a540-ae46a686642e`
- Recipe version：`1.3.0`
- Recipe SHA-256：`909cdbe2e17a6bbaae3361fb7f1063979b2c2cf5856d4f6b5e1e00e070544048`
- Package name：`minimax_h3_reference_video_1_3_0`
- Package source：local builtin package（本地路径已隐去）
- Dynamic binding targets：
  `14.height`, `14.prompt`, `14.width`, `15.noise_seed`, `22.value`,
  `14.ref_images.ref_image_0..8`, `14.ref_videos.ref_video_0..2`,
  `14.ref_video_audios.ref_video_audio_0..2`, `14.ref_audios.ref_audio_0..2`,
  `24/28/29/30/31/32/33/34/35.image`, `40/42/44.file`, `50/51/52.audio`

## Live cases

### Case 1 — 3 reference images

**DEV-016C: PASS — USER-VERIFIED LIVE PASS.**

- Evidence source: Product owner manual live acceptance
- Mode: MiniMax H3 REF2VA FAST
- Parameters: 3 reference images, 5 seconds, 864×480, fixed seed
- Reference order: A → B → C
- Final binding order: `ref_image_0 → A`, `ref_image_1 → B`, `ref_image_2 → C`
- ComfyUI actual execution, Task success, MP4 output, Asset recovery, Restart, Task History and Load to Studio: **USER-VERIFIED PASS**
- Machine-readable Task identifier: **NOT RECORDED**

The current database contains an earlier real 3-image REF2VA success, but it is not a substitute for this case because it used the quality package, 3 seconds and 960×544:

- Task：`tsk_084ccbc1-de03-4e80-abb9-9183b3a64641`
- Status：`SUCCEEDED`
- Manifest order：
  `ast_38074582-4be0-4fe0-93e6-92ca055c10cd`
  → `ast_27734914-a81c-42c1-b552-55d78763fd9d`
  → `ast_732ab0ae-7799-4fc9-aeaf-abf6945ca1d9`
- Snapshot and compiled workflow preserved the same order at `ref_image_0`, `ref_image_1`, `ref_image_2`
- Output：`ast_6c3680f3-d73c-463c-a7d1-c0a5da068e03`, MP4, 960×544

The earlier WebView automation limitation did not reproduce a product defect. The database task below remains historical machine evidence; it is not presented as the machine identifier for this final FAST acceptance.

### Case 2 — 2 images + 1 audio

**PASS — real ComfyUI execution.**

- Task：`tsk_3adfee36-4e2f-4e89-96c6-37f850edd9e7`
- Batch：`pbt_656e1187e9d84d2ea7117a55ed73b8f8`
- ComfyUI prompt ID：`05edcc32-d1c3-4c94-a320-21992015a289`
- Parameters：5 seconds, 864×480, fixed seed `749183511376195`
- Images, in order：
  `ast_179ce42b-1c9e-484b-bb26-6538fa9c370c`
  → `ast_c75cd100-3c9a-4800-a45b-65c9f8919381`
- Audio：`ast_2455e48b-ec1d-493e-b41f-38c395fdc9a2`
- Snapshot：`snp_c0af6bc4-6280-4fd9-81da-48144c7957bc`
- Compiled slots: image 0 → node 24, image 1 → node 28, audio 0 → node 50
- No optional placeholder remained in the compiled workflow
- Status：`SUCCEEDED`
- Output asset：`ast_76260d7a-5228-4e51-a5a5-f4a5f513f01e`, MP4, 864×480, about 5 seconds

### Case 3 — 1 video + 2 images

**PASS — real ComfyUI execution.**

- Task：`tsk_5dab1cd2-f60d-4a2e-bd33-2daad4448cbd`
- Batch：`pbt_048ec4b571f94b35aeeb398bea66e68a`
- ComfyUI prompt ID：`d3cfba81-5b0b-4427-a521-57be37a33f2a`
- Parameters：5 seconds, 864×480, fixed seed `329198012692815`
- Images, in order：
  `ast_179ce42b-1c9e-484b-bb26-6538fa9c370c`
  → `ast_c75cd100-3c9a-4800-a45b-65c9f8919381`
- Video：`ast_76260d7a-5228-4e51-a5a5-f4a5f513f01e`
- Snapshot：`snp_0cddede1-3b99-497f-9cc5-4ad362ee8200`
- Compiled slots: image 0 → node 24, image 1 → node 28, video 0 → node 40 / components node 41
- Optional video slots were cleared; no optional placeholder remained
- Status：`SUCCEEDED`
- Output asset：`ast_11b549f1-a592-444c-9ad2-7536364fa225`, MP4, 864×480, about 5 seconds

## DEV-016D — H3 QUALITY 20-Step Live Validation

**PASS — evidence recovered from the current Task, Snapshot, Task Events and Asset rows.**

- Task：`tsk_496106ce-44f3-4d74-8395-6deb6bb3ee40`
- Status：`SUCCEEDED`
- ComfyUI prompt ID：`3d64394e-eb8e-4b84-8232-bf4588ebcf30`
- Workflow ID：`wfl_minimax_h3_fl2va_first_last_quality`
- Workflow version ID：`wfv_05eb0fc3-04dd-47b9-a7fe-11077800a35c`
- Workflow version：`2.0.0`
- Workflow SHA-256：`96d98f74684cc3f76d028a182829a6f2a7ac3c0d37173a6182edc68b63ced992`
- Recipe ID：`rcp_0da72110-47a5-4a90-96d1-4f601d11b0d0`
- Recipe version：`2.0.0`
- Recipe SHA-256：`761760bb34f7ffd30e448e28ebc5ea43ecaac110c6cbf5ebf932aaf5bc3636ab`
- Package：`minimax_h3_fl2va_first_last_quality_2_0_0`
- Parameters：fixed seed `398926132886102`, `1` second, `736×416`
- Sampling：compiled node `23` recorded `steps = 20`
- Dynamic targets：`14.first_frame`, `14.height`, `14.last_frame`, `14.prompt`, `14.width`, `15.noise_seed`, `22.value`, `24.image`, `28.image`
- Snapshot：`snp_0781a464-8a41-4e20-a845-f1d604ae206f`
- Output Asset：`ast_fba9ae5d-7a74-40bb-8291-386473d7325d`, MP4, `736×416`, `1625 ms`
- Task Events：submission prompt ID, node execution, progress `1/20` through `20/20`, collection and success are all present
- Compiled workflow: first/last frame inputs were concrete task-scoped uploaded identities; internal optional placeholders were absent
- Runtime Provenance：app `0.3.0`, build `d8dabc9a104f7b14cfd041cbc62c5cfde53678ac`, package/workflow/recipe hashes and dynamic targets persisted

This is a QUALITY first/last-frame validation record. It does not satisfy the separate DEV-016C exact target of three reference images using FAST at 5 seconds and 864×480.

## Native playback

**PASS for Case 3 output.** The task detail opened the generated video in AI Studio's native preview. After clicking the native `播放` control, the accessibility state changed to `暂停`, confirming playback started inside AI Studio.

## ReferenceManifest negative validation

These are automated pre-submit guards; no negative case was sent to ComfyUI:

- Missing item (`[A,B,C]` manifest vs `[A,B]` values)：**PASS**, `REFERENCE_MAPPING_INCOMPLETE`
- Wrong order：**PASS**, exact order comparison returns `REFERENCE_MAPPING_INCOMPLETE`
- Duplicate asset ID：**PASS**, exact order/multiplicity comparison returns `REFERENCE_MAPPING_INCOMPLETE`
- Missing or wrong-type asset before upload：**PASS**, `generation_input_preparer::tests::rejects_missing_wrong_type_and_cross_project_media_before_upload`
- Invalid input key：**PASS**, `compiler::workflow_compiler::tests::rejects_unknown_user_input`
- Internal placeholder invariant before `/prompt`：**PASS**, `final_workflow_rejects_internal_placeholders_before_prompt_submission`

Targeted results:

- `cargo test reference_manifest -- --test-threads=1`：3 passed
- `cargo test generation_input_preparer -- --test-threads=1`：3 passed
- `cargo test workflow_compiler -- --test-threads=1`：16 passed
- `cargo test generation_service -- --test-threads=1`：9 passed

## Runtime provenance

Both current FAST live tasks contain non-empty app version, build commit, workflow ID/version/SHA, recipe ID/version/SHA, package name/source and dynamic binding targets. The task detail Runtime Diagnostics showed these fields before the restart. The persisted database rows still contain them after the restart process was relaunched.

## Restart recovery

- Database persistence: PASS — current Tasks, Snapshots, ordered reference IDs and generated MP4 Assets remain readable after relaunch.
- Product owner manual live acceptance: **USER-VERIFIED PASS** for Restart, Task History and `加载到创作` / Load to Studio order recovery.
- The previous `NOT RECORDED` state reflected desktop WebView automation visibility limitations, not a reproduced product defect.

## Automated regression baseline

No product source code changed in this validation run. The current HEAD baseline had already passed:

- `cargo fmt --all -- --check`：PASS
- `cargo check`：PASS
- `cargo test -- --test-threads=1`：422 passed, 0 failed
- `pnpm test`：46 files / 152 tests passed, 0 failed
- `pnpm build`：PASS
- `git diff --check`：PASS before this documentation-only change

The final local Release Candidate build was completed after this live-validation record. No tag, GitHub Release or binary upload was performed.

## DEV-017B-RECONCILE final evidence

- Source RC SHA: `c589938d57e80aa13e1abdd90eea0ab8b743ff6a`
- `pnpm tauri build`: **PASS**
- Embedded build commit: **PASS** — the standalone executable contains the Source RC SHA.
- Artifact SHA-256 and byte counts: recorded in `docs/RELEASE_SHA256_0.3.0.txt`.
- Installation, normal launch, close, uninstall and reinstall smoke: **PASS** in a controlled temporary install root.
- Product-owner manual acceptance remains the evidence source for live/UI/restart/load-to-studio entries without machine-readable identifiers; those entries are **USER-VERIFIED PASS**, not fabricated machine evidence.

## Final decision

**DEV-016C PASS — USER-VERIFIED LIVE PASS.**

Case 2、Case 3、原生播放、DEV-016D QUALITY 20-Step、Runtime Provenance、Snapshot/Asset 数据持久化、Restart、Load to Studio 顺序以及负向自动化守卫均有真实或可复核证据；DEV-016C 最终 FAST 三图字段由产品负责人人工验收确认，机器可追溯 ID 未记录。
