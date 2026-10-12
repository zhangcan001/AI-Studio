import { describe, expect, it } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type { ShotView } from "../../types/shot";
import {
  assetAllowedForBindKey,
  encodeExactSelectionRef,
  groupAssignmentsByShot,
  mergeShotBindings,
  parseBatchBindTable,
  recipeMediaSlots,
  resolveBatchBindRows,
  shotsReadyForBatchBind,
} from "./shotBatchAssetBind";

function recipe(
  partial: Partial<RecipeViewModel> & Pick<RecipeViewModel, "recipeId" | "mode">,
): RecipeViewModel {
  return {
    workflowId: "wf",
    workflowVersionId: partial.workflowVersionId ?? "wfv-1",
    recipeId: partial.recipeId,
    name: partial.name ?? partial.recipeId,
    category: "video",
    mode: partial.mode,
    persistentInputs: partial.persistentInputs ?? true,
    selectionRef: partial.selectionRef,
    fields: partial.fields ?? [
      { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
      { key: "first_frame", type: "image", label: "首帧", required: true },
    ],
  };
}

function shot(
  id: string,
  ordinal: number,
  config?: { workflowVersionId: string; recipeId: string },
): ShotView {
  return {
    id,
    projectId: "p1",
    ordinal,
    name: `镜头${ordinal + 1}`,
    promptText: "",
    createdAt: "",
    updatedAt: "",
    status: "DRAFT",
    imageStatus: "IDLE",
    videoStatus: "IDLE",
    stageConfigs: config
      ? [
          {
            stage: "video",
            workflowVersionId: config.workflowVersionId,
            recipeId: config.recipeId,
            scalarValues: {},
            updatedAt: "",
          },
        ]
      : [],
    referenceAssets: [],
    generationLinks: [],
  };
}

describe("shotBatchAssetBind", () => {
  it("encodes ExactGeneratorSelection like Rust product selection_ref", () => {
    const ref = encodeExactSelectionRef("wfv:旧版本/🖼", "recipe:准确配方");
    expect(ref).toBe(
      "generator:v1:" +
        Array.from(
          new TextEncoder().encode(JSON.stringify(["wfv:旧版本/🖼", "recipe:准确配方"])),
          (b) => b.toString(16).padStart(2, "0"),
        ).join(""),
    );
  });

  it("parses TSV with header and 1-based ordinals", () => {
    const rows = parseBatchBindTable(
      "shot_ordinal\tinput_key\tasset_id\n1\tfirst_frame\ta1\n2\treference_images\ta2\n",
    );
    expect(rows).toHaveLength(2);
    expect(rows[0]).toMatchObject({
      shotOrdinal: 1,
      inputKey: "first_frame",
      assetId: "a1",
      errors: [],
    });
    expect(rows[1].shotOrdinal).toBe(2);
  });

  it("rejects unknown keys and missing cells", () => {
    const rows = parseBatchBindTable("1\tprompt\tx\n\tfirst_frame\t\n");
    expect(rows[0].errors.some((e) => e.includes("不支持的槽位"))).toBe(true);
    expect(rows[1].errors.length).toBeGreaterThan(0);
  });

  it("resolves rows against shots with video stage configs", () => {
    const catalog = [
      recipe({
        recipeId: "i2v",
        mode: "I2V",
        selectionRef: "opaque-i2v",
      }),
    ];
    const shots = [
      shot("s1", 0, { workflowVersionId: "wfv-1", recipeId: "i2v" }),
      shot("s2", 1),
    ];
    const resolved = resolveBatchBindRows(
      parseBatchBindTable("1\tfirst_frame\timg1\n2\tfirst_frame\timg2\n"),
      shots,
      catalog,
    );
    expect(resolved[0].errors).toEqual([]);
    expect(resolved[0].selectionRef).toBe("opaque-i2v");
    expect(resolved[1].errors.some((e) => e.includes("尚未配置视频"))).toBe(
      true,
    );
  });

  it("merges replace for first_frame and append for reference_images", () => {
    const replaced = mergeShotBindings(
      [{ inputKey: "first_frame", ordinal: 0, assetId: "old" }],
      "first_frame",
      "new",
    );
    expect(replaced.inputs).toEqual([
      { inputKey: "first_frame", ordinal: 0, assetId: "new" },
    ]);
    const appended = mergeShotBindings(
      [{ inputKey: "reference_images", ordinal: 0, assetId: "a" }],
      "reference_images",
      "b",
    );
    expect(appended.inputs.map((i) => i.assetId)).toEqual(["a", "b"]);
  });

  it("enforces REF2VA combination ceilings on merge", () => {
    const existing = Array.from({ length: 9 }, (_, ordinal) => ({
      inputKey: "reference_images" as const,
      ordinal,
      assetId: `i${ordinal}`,
    }));
    const overflow = mergeShotBindings(existing, "reference_images", "extra");
    expect(overflow.error).toMatch(/最多 9/);
    const audioOnly = mergeShotBindings([], "reference_audios", "aud");
    expect(audioOnly.error).toMatch(/不能只有音频/);
  });

  it("only allows external source categories on formal image/video/audio slots", () => {
    expect(assetAllowedForBindKey("first_frame", "source_image")).toBe(true);
    expect(assetAllowedForBindKey("first_frame", "generated_image", "task")).toBe(
      false,
    );
    expect(assetAllowedForBindKey("reference_videos", "source_video")).toBe(
      true,
    );
    expect(
      assetAllowedForBindKey("reference_videos", "generated_video", "task"),
    ).toBe(false);
  });

  it("lists bindable shots and groups OCC plans without inventing a queue", () => {
    const catalog = [
      recipe({
        recipeId: "fl",
        mode: "FIRST_LAST",
        fields: [
          { key: "prompt", type: "textarea", label: "提示词", required: true, default: "" },
          { key: "first_frame", type: "image", label: "首帧", required: true },
          { key: "last_frame", type: "image", label: "尾帧", required: true },
        ],
      }),
    ];
    const shots = [
      shot("s1", 0, { workflowVersionId: "wfv-1", recipeId: "fl" }),
      shot("s2", 1, { workflowVersionId: "wfv-1", recipeId: "fl" }),
    ];
    const ready = shotsReadyForBatchBind(shots, catalog);
    expect(ready).toHaveLength(2);
    expect(recipeMediaSlots(ready[0].recipe)).toEqual([
      "first_frame",
      "last_frame",
    ]);
    const plans = groupAssignmentsByShot(
      resolveBatchBindRows(
        parseBatchBindTable(
          "1\tfirst_frame\ta\n1\tlast_frame\tb\n2\tfirst_frame\tc\n",
        ),
        shots,
        catalog,
      ),
    );
    expect(plans).toHaveLength(2);
    expect(plans[0].assignments).toHaveLength(2);
    expect(plans[1].assignments[0].assetId).toBe("c");
  });
});
