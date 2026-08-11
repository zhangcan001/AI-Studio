import type { RecipeField, RecipeViewModel } from "../../types/generation";

export type IntegerRecipeField = Extract<RecipeField, { type: "integer" }>;
export type ResolutionFieldKey = "width" | "height";

export interface ResolutionFields {
  width: IntegerRecipeField;
  height: IntegerRecipeField;
}

export interface ResolutionValidationResult {
  ok: boolean;
  errors: Partial<Record<ResolutionFieldKey, string>>;
}

export function resolutionFields(recipe: RecipeViewModel): ResolutionFields | undefined {
  const width = exactIntegerField(recipe, "width");
  const height = exactIntegerField(recipe, "height");
  return width && height ? { width, height } : undefined;
}

export function resolutionContractError(recipe: RecipeViewModel): string | undefined {
  const width = exactIntegerField(recipe, "width");
  if (!width) return "Recipe 缺少 key 为 `width` 的 integer 字段。";
  const height = exactIntegerField(recipe, "height");
  if (!height) return "Recipe 缺少 key 为 `height` 的 integer 字段。";
  if (!width.required) return "Recipe 的 `width` 必须是必填 integer 字段。";
  if (!height.required) return "Recipe 的 `height` 必须是必填 integer 字段。";
  return undefined;
}

export function validateResolution(
  recipe: RecipeViewModel,
  width: number | undefined,
  height: number | undefined,
): ResolutionValidationResult {
  const fields = resolutionFields(recipe);
  if (!fields) {
    return {
      ok: false,
      errors: {
        width: resolutionContractError(recipe) ?? "Recipe 缺少合法的 width/height 字段。",
        height: resolutionContractError(recipe) ?? "Recipe 缺少合法的 width/height 字段。",
      },
    };
  }

  const errors: Partial<Record<ResolutionFieldKey, string>> = {};
  const widthError = validateResolutionValue(fields.width, width, "宽度");
  const heightError = validateResolutionValue(fields.height, height, "高度");
  if (widthError) errors.width = widthError;
  if (heightError) errors.height = heightError;
  return { ok: Object.keys(errors).length === 0, errors };
}

export function isResolutionAllowedByRecipe(
  recipe: RecipeViewModel,
  width: number,
  height: number,
): boolean {
  return validateResolution(recipe, width, height).ok;
}

export function validateResolutionValue(
  field: IntegerRecipeField,
  value: number | undefined,
  label: string,
): string | undefined {
  if (value === undefined || !Number.isFinite(value) || !Number.isInteger(value)) {
    return `${label}必须是整数。`;
  }
  if (value <= 0) return `${label}必须大于 0。`;
  if (field.min !== undefined && value < field.min) {
    return `${label}不能小于 ${field.min}。`;
  }
  if (field.max !== undefined && value > field.max) {
    return `${label}不能大于 ${field.max}。`;
  }
  if (field.step !== undefined && field.step > 0 && value % field.step !== 0) {
    return `${label}必须是 ${field.step} 的倍数。`;
  }
  return undefined;
}

function exactIntegerField(recipe: RecipeViewModel, key: ResolutionFieldKey): IntegerRecipeField | undefined {
  return recipe.fields.find((field) => field.key === key && field.type === "integer") as IntegerRecipeField | undefined;
}
