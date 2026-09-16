// @vitest-environment jsdom
import { useState } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import { DynamicFormRenderer, validateRecipeValues } from "./DynamicFormRenderer";

afterEach(cleanup);
const imported: RecipeViewModel = {
  workflowId: "wfl_imported", workflowVersionId: "wfv_exact", recipeId: "rcp_exact",
  name: "AITUDOU-MiniMax H3 lightx2v 8step 首尾帧", category: "video", mode: "custom", outputTypes: ["video"],
  fields: [
    { key: "width", label: "宽度", type: "integer", required: true, default: 960 },
    { key: "height", label: "高度", type: "integer", required: true, default: 544 },
  ],
};
function Form({ recipe = imported }: { recipe?: RecipeViewModel }) {
  const [values, setValues] = useState<GenerationValues>({
    width: { type: "integer", value: 960 }, height: { type: "integer", value: 544 },
  });
  return <><DynamicFormRenderer recipe={recipe} values={values} validationErrors={{}}
    onChange={(key, value) => { if (value) setValues(current => ({ ...current, [key]: value })); }}
    onGenerate={() => {}} projectId="project" />
    <output data-testid="values">{JSON.stringify(values)}</output></>;
}
describe("imported H3 resolution form", () => {
  it.each(["111video_minimax_h3_i2v", "video_minimax_h3_t2v (2)"])("recognizes the actual imported name %s", (name) => {
    render(<Form recipe={{ ...imported, name }} />);
    expect(screen.queryAllByRole("spinbutton")).toHaveLength(0);
    expect(screen.getAllByRole("option")).toHaveLength(14);
  });
  it("renders all 14 presets instead of width/height inputs and updates both values", async () => {
    render(<Form />);
    expect(screen.queryAllByRole("spinbutton")).toHaveLength(0);
    expect(screen.getAllByRole("option")).toHaveLength(14);
    await userEvent.selectOptions(screen.getByRole("combobox"), "h3-2-0mp-16x9");
    expect(JSON.parse(screen.getByTestId("values").textContent!)).toEqual({
      width: { type: "integer", value: 1920 }, height: { type: "integer", value: 1088 },
    });
  });
  it("does not change non-H3 video or image forms", () => {
    const view = render(<Form recipe={{ ...imported, name: "Other video" }} />);
    expect(screen.getAllByRole("spinbutton")).toHaveLength(2);
    view.rerender(<Form recipe={{ ...imported, name: "video_minimax_h30_i2v" }} />);
    expect(screen.getAllByRole("spinbutton")).toHaveLength(2);
    view.rerender(<Form recipe={{ ...imported, outputTypes: ["image"] }} />);
    expect(screen.getAllByRole("spinbutton")).toHaveLength(2);
  });
  it("rejects unsupported imported H3 sizes, but leaves non-H3 validation unchanged", () => {
    const values: GenerationValues = { width: { type: "integer", value: 1024 }, height: { type: "integer", value: 576 } };
    expect(validateRecipeValues(imported, values).width).toContain("H3");
    expect(validateRecipeValues({ ...imported, name: "111video_minimax_h3_i2v" }, values).width).toContain("H3");
    expect(validateRecipeValues({ ...imported, name: "Other video" }, values)).toEqual({});
  });
});
