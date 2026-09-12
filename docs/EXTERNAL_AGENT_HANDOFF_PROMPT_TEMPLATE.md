# External agent instruction template — AI Studio ProductionHandoffV1

Copy the instruction below to any external agent. Give it the current AI
Studio project ID and your source material. The agent produces structured
production input; AI Studio does not author the narrative or prompts for it.

> Return **only** one JSON object conforming to
> `production-handoff-v1.schema.json` (`schemaVersion: 1`). Do not add Markdown,
> explanation, comments, or unknown fields. Use the supplied exact `projectId`.
> Organize the output as Series → Episodes → Scenes → Shots, with stable,
> source-owned `externalId` values, names, descriptions, and positive ordinals.
> Supply `imagePrompt` and `videoPrompt` only when the source material supports
> them. Do not invent AI Studio internal IDs. Do not guess Asset IDs,
> `workflowVersionId`, or `recipeId`; omit `assetRefs` or `stages` when exact
> values have not been supplied. Never infer a workflow or recipe by display
> name. Keep the same `source.agent` and stable `source.revision` for an exact
> replay; use a new revision for changed content. Keep at most 500 Shots.

Replace the placeholders in the [canonical example](examples/production-handoff-v1.example.json)
with the current project's ID and any exact active workflow/recipe pair before
preflight. Do not submit placeholder Asset or workflow IDs. AI Studio's server
performs the final project, asset, workflow, recipe, size, and uniqueness
validation; preflight is read-only and confirmation does not start production.
