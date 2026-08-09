import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import { parseBatchTaskList } from "./batchImport";

const catalog: RecipeViewModel[] = [
  {
    workflowId: "wfl_kera2",
    workflowVersionId: "wfv_kera2",
    recipeId: "rcp_kera2",
    name: "Kera2 Image",
    category: "image",
    mode: "text_to_image",
    fields: [],
  },
  {
    workflowId: "wfl_h3",
    workflowVersionId: "wfv_h3",
    recipeId: "rcp_h3",
    name: "MiniMax H3 Video",
    category: "video",
    mode: "image_to_video",
    fields: [],
  },
];

describe("batch task-list import", () => {
  it("parses typed generation values and resolves the current catalog name", () => {
    const result = parseBatchTaskList(
      JSON.stringify({
        schemaVersion: 1,
        items: [
          {
            workflowVersionId: "wfv_kera2",
            recipeId: "rcp_kera2",
            values: {
              prompt: { type: "string", value: "portrait" },
              seed: { type: "seed_random" },
            },
          },
        ],
      }),
      catalog,
    );

    expect(result).toEqual([
      {
        workflowName: "Kera2 Image",
        workflowVersionId: "wfv_kera2",
        recipeId: "rcp_kera2",
        values: {
          prompt: { type: "string", value: "portrait" },
          seed: { type: "seed_random" },
        },
      },
    ]);
  });

  it("rejects unavailable recipes and malformed values before submission", () => {
    expect(() =>
      parseBatchTaskList(
        JSON.stringify({
          schemaVersion: 1,
          items: [{ workflowVersionId: "missing", recipeId: "missing", values: {} }],
        }),
        catalog,
      ),
    ).toThrow(/WORKFLOW_UNAVAILABLE/);

    expect(() =>
      parseBatchTaskList(
        JSON.stringify({
          schemaVersion: 1,
          items: [{ workflowVersionId: "wfv_h3", recipeId: "rcp_h3", values: { prompt: "bad" } }],
        }),
        catalog,
      ),
    ).toThrow(/VALUES_INVALID/);
  });
});
