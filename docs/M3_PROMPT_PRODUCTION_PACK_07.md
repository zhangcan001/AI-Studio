# M3 Prompt & Production Pack 07 — source implementation started

Pack 06 code verification is complete and the local third-runtime boundary remains `THIRD_RUNTIME_INPUT_REQUIRED`, so Pack 07 source work has started without adding a new migration.

The first source slice is `src/features/prompts/promptLibrary.ts`. It establishes the reusable prompt contracts for:

- project-scoped prompt records and ordered versions;
- normalized prompt text and sequential version creation;
- prompt-version line comparison;
- applying a version only to an exact Recipe textarea field;
- converting prompt versions into text variant values for future Experiment Planner integration.

The UI, Tauri persistence commands, project scoping enforcement, and migration `009_prompt_library.sql` are intentionally deferred to the next Pack 07 implementation slice. Migrations `001–008` remain unchanged and v0.2.0 remains frozen.
