import { expect, it } from "vitest";
import { explicitEntityMentions } from "../create/shotPromptParser";
import cases from "./__fixtures__/folderEntityMentions.json";
import type { StoryEntity } from "../../types/storyEntity";

it.each(cases)("shares exact entity mention semantics: $prompt", ({ names, prompt, expected }) => {
  const entities: StoryEntity[] = names.map((name, i) => ({ id: `entity-${i}`, projectId: "project",
    name, kind: "character", description: "", coverAssetId: null, imageAssetIds: [], createdAt: "", updatedAt: "" }));
  const actual = explicitEntityMentions(prompt, entities);
  if (expected === null) expect(actual.error).toBeTruthy();
  else { expect(actual.error).toBeUndefined(); expect(actual.entities.map(e => e.name)).toEqual(expected); }
});
