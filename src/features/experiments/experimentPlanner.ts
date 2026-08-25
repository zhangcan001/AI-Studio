import type { DraftValue, GenerationValues, RecipeField, RecipeViewModel } from "../../types/generation";
import { runtimeKindFor } from "../runtime/pack05";

export type ExperimentVariantValue =
  | Extract<DraftValue, { type: "string" }>
  | Extract<DraftValue, { type: "integer" }>
  | Extract<DraftValue, { type: "seed_fixed" }>;

export interface ExperimentDimension {
  fieldKey: string;
  values: ExperimentVariantValue[];
}

export interface ExperimentChange {
  fieldKey: string;
  fieldLabel: string;
  value: string;
}

export interface ExperimentPlanItem {
  id: string;
  ordinal: number;
  values: GenerationValues;
  changes: ExperimentChange[];
  seed?: string;
}

export interface ExperimentPlan {
  workflowVersionId: string;
  recipeId: string;
  workflowName: string;
  baseValues: GenerationValues;
  dimensions: ExperimentDimension[];
  items: ExperimentPlanItem[];
  videoWarning: boolean;
  frozenAt: string;
}

export interface ExperimentContext {
  recipe: RecipeViewModel;
  baseValues: GenerationValues;
}

export interface ExperimentPlanBuildResult {
  plan?: ExperimentPlan;
  issues: string[];
}

export interface SeedFieldDefinition {
  key: string;
  minValue?: string | null;
  maxValue?: string | null;
}

export type SeedSource = (field: SeedFieldDefinition, index: number) => string;

const MAX_DIMENSIONS = 2;
const MAX_TEXT_VARIANTS = 8;
const MAX_ITEMS = 24;

export function isExperimentVariantField(field: RecipeField): field is Extract<RecipeField, { type: "textarea" | "integer" | "seed" }> {
  return field.type === "textarea" || field.type === "integer" || field.type === "seed";
}

export function experimentVariantFields(recipe: RecipeViewModel): Array<Extract<RecipeField, { type: "textarea" | "integer" | "seed" }>> {
  return recipe.fields.filter(isExperimentVariantField);
}

export function freezeSeedVariants(
  field: SeedFieldDefinition,
  count: number,
  source: SeedSource = randomSeedForRange,
): { values: Array<Extract<DraftValue, { type: "seed_fixed" }>>; issues: string[] } {
  const issues: string[] = [];
  if (!Number.isInteger(count) || count < 1 || count > MAX_ITEMS) {
    return { values: [], issues: ["随机 Seed 数量必须是 1–24。"] };
  }
  const min = parseSeed(field.minValue);
  const max = parseSeed(field.maxValue);
  if (min === undefined || max === undefined || min > max) {
    return { values: [], issues: ["当前配方没有可用的 Seed 合法范围。"] };
  }
  const values: Array<Extract<DraftValue, { type: "seed_fixed" }>> = [];
  const seen = new Set<string>();
  for (let index = 0; index < count; index += 1) {
    const value = source(field, index);
    const numeric = parseSeed(value);
    if (numeric === undefined || numeric < min || numeric > max) {
      issues.push("生成的随机 Seed 超出当前配方范围。");
      continue;
    }
    const fixed = numeric.toString();
    if (seen.has(fixed)) {
      issues.push("随机 Seed 发生重复，请重新冻结实验计划。");
      continue;
    }
    seen.add(fixed);
    values.push({ type: "seed_fixed", value: fixed });
  }
  return { values, issues };
}

export function freezeRandomSeeds(
  recipe: RecipeViewModel,
  baseValues: GenerationValues,
  source: SeedSource = randomSeedForRange,
): { values: GenerationValues; issues: string[] } {
  const nextValues = cloneGenerationValues(baseValues);
  const issues: string[] = [];
  for (const field of recipe.fields) {
    if (field.type !== "seed" || nextValues[field.key]?.type !== "seed_random") continue;
    const frozen = freezeSeedVariants(field, 1, source);
    issues.push(...frozen.issues);
    if (frozen.values[0]) nextValues[field.key] = frozen.values[0];
  }
  return { values: nextValues, issues };
}

export function buildExperimentPlan({
  recipe,
  baseValues,
  dimensions,
  seedSource,
  now = new Date().toISOString(),
}: {
  recipe: RecipeViewModel;
  baseValues: GenerationValues;
  dimensions: ExperimentDimension[];
  seedSource?: SeedSource;
  now?: string;
}): ExperimentPlanBuildResult {
  const issues: string[] = [];
  if (!dimensions.length) issues.push("请至少选择一个实验参数。");
  if (dimensions.length > MAX_DIMENSIONS) issues.push("第一版实验最多支持两个变体维度。");

  const fieldMap = new Map(recipe.fields.map((field) => [field.key, field]));
  const seenKeys = new Set<string>();
  for (const dimension of dimensions) {
    const field = fieldMap.get(dimension.fieldKey);
    if (!field || !isExperimentVariantField(field)) {
      issues.push(`字段“${dimension.fieldKey}”不支持组合实验。`);
      continue;
    }
    if (seenKeys.has(dimension.fieldKey)) {
      issues.push(`字段“${field.label}”不能重复作为实验维度。`);
    }
    seenKeys.add(dimension.fieldKey);
    if (!dimension.values.length) {
      issues.push(`字段“${field.label}”至少需要一个变体值。`);
    }
    if (field.type === "textarea" && dimension.values.length > MAX_TEXT_VARIANTS) {
      issues.push(`文本字段“${field.label}”最多支持 ${MAX_TEXT_VARIANTS} 个变体。`);
    }
    for (const value of dimension.values) {
      validateVariantValue(field, value, issues);
    }
  }

  const frozen = freezeRandomSeeds(recipe, baseValues, seedSource);
  issues.push(...frozen.issues);
  const itemCount = dimensions.reduce((count, dimension) => count * Math.max(1, dimension.values.length), 1);
  if (itemCount < 1 || itemCount > MAX_ITEMS) issues.push(`本次实验将生成 ${itemCount} 个任务，限制为 1–${MAX_ITEMS} 个。`);
  if (issues.length) return { issues: uniqueStrings(issues) };

  const combinations = cartesianProduct(dimensions.map((dimension) => dimension.values));
  const items = combinations.map((combination, index) => {
    const values = cloneGenerationValues(frozen.values);
    const changes: ExperimentChange[] = [];
    let seed: string | undefined;
    combination.forEach((value, dimensionIndex) => {
      const dimension = dimensions[dimensionIndex];
      const field = fieldMap.get(dimension.fieldKey)!;
      values[field.key] = cloneDraftValue(value);
      changes.push({ fieldKey: field.key, fieldLabel: field.label, value: displayVariantValue(value) });
      if (value.type === "seed_fixed") seed = value.value;
    });
    if (!seed) {
      const seedField = recipe.fields.find((field): field is Extract<RecipeField, { type: "seed" }> => field.type === "seed");
      const seedValue = seedField ? values[seedField.key] : undefined;
      if (seedValue?.type === "seed_fixed") seed = seedValue.value;
    }
    return {
      id: `experiment-item-${index + 1}`,
      ordinal: index,
      values,
      changes,
      seed,
    };
  });

  return {
    issues: [],
    plan: {
      workflowVersionId: recipe.workflowVersionId,
      recipeId: recipe.recipeId,
      workflowName: recipe.name,
      baseValues: frozen.values,
      dimensions: dimensions.map((dimension) => ({
        fieldKey: dimension.fieldKey,
        values: dimension.values.map(cloneDraftValue),
      })),
      items,
      videoWarning: runtimeKindFor(recipe) === "video",
      frozenAt: now,
    },
  };
}

export function removeExperimentPlanItem(plan: ExperimentPlan, itemId: string): ExperimentPlan {
  const items = plan.items
    .filter((item) => item.id !== itemId)
    .map((item, ordinal) => ({ ...item, ordinal }));
  return { ...plan, items };
}

export interface SnapshotDiffEntry {
  fieldKey: string;
  before: string;
  after: string;
}

export function snapshotDiff(
  before: GenerationValues,
  after: GenerationValues,
  fieldLabels: Readonly<Record<string, string>> = {},
): SnapshotDiffEntry[] {
  const keys = new Set([...Object.keys(before), ...Object.keys(after)]);
  return [...keys]
    .sort()
    .flatMap((fieldKey) => {
      const left = before[fieldKey];
      const right = after[fieldKey];
      if (draftValuesEqual(left, right)) return [];
      return [{ fieldKey: fieldLabels[fieldKey] ?? fieldKey, before: safeDraftValueLabel(left), after: safeDraftValueLabel(right) }];
    });
}

export function experimentTaskDurationMs(createdAt?: string, finishedAt?: string, startedAt?: string): number | undefined {
  if (!finishedAt) return undefined;
  const start = startedAt ?? createdAt;
  if (!start) return undefined;
  const duration = Date.parse(finishedAt) - Date.parse(start);
  return Number.isFinite(duration) && duration >= 0 ? duration : undefined;
}

export function displayVariantValue(value: DraftValue | undefined): string {
  return safeDraftValueLabel(value);
}

function validateVariantValue(field: Extract<RecipeField, { type: "textarea" | "integer" | "seed" }>, value: ExperimentVariantValue, issues: string[]) {
  if (field.type === "textarea") {
    if (value.type !== "string") issues.push(`字段“${field.label}”的变体类型不匹配。`);
    else if (!value.value.trim()) issues.push(`字段“${field.label}”不能使用空文本变体。`);
    return;
  }
  if (field.type === "integer") {
    if (value.type !== "integer") {
      issues.push(`字段“${field.label}”的变体类型不匹配。`);
      return;
    }
    if (!Number.isSafeInteger(value.value)) issues.push(`字段“${field.label}”的变体必须是安全整数。`);
    if (field.min !== undefined && value.value < field.min) issues.push(`字段“${field.label}”的变体不能小于 ${field.min}。`);
    if (field.max !== undefined && value.value > field.max) issues.push(`字段“${field.label}”的变体不能大于 ${field.max}。`);
    return;
  }
  if (value.type !== "seed_fixed") {
    issues.push(`字段“${field.label}”的变体类型不匹配。`);
    return;
  }
  const numeric = parseSeed(value.value);
  const min = parseSeed(field.minValue);
  const max = parseSeed(field.maxValue);
  if (numeric === undefined || min === undefined || max === undefined || numeric < min || numeric > max) {
  issues.push(`字段“${field.label}”的 Seed 不在配方合法范围内。`);
  }
}

function cartesianProduct<T>(dimensions: T[][]): T[][] {
  return dimensions.reduce<T[][]>((products, dimension) => products.flatMap((product) => dimension.map((value) => [...product, value])), [[]]);
}

function cloneGenerationValues(values: GenerationValues): GenerationValues {
  return JSON.parse(JSON.stringify(values)) as GenerationValues;
}

function cloneDraftValue(value: ExperimentVariantValue): ExperimentVariantValue {
  return { ...value };
}

function draftValuesEqual(left: DraftValue | undefined, right: DraftValue | undefined): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

function safeDraftValueLabel(value: DraftValue | undefined): string {
  if (!value) return "未设置";
  switch (value.type) {
    case "string":
      return value.value ? (value.value.length > 80 ? `${value.value.slice(0, 77)}…` : value.value) : "（空）";
    case "integer":
      return String(value.value);
    case "number":
      return String(value.value);
    case "seed_random":
      return "随机 Seed";
    case "seed_fixed":
      return value.value;
    case "image_asset":
    case "video_asset":
    case "audio_asset":
      return "素材已绑定";
    case "image_assets":
      return `图片素材 × ${value.assetIds.length}`;
    case "video_assets":
      return `视频素材 × ${value.assetIds.length}`;
    case "audio_assets":
      return `音频素材 × ${value.assetIds.length}`;
  }
}

function parseSeed(value?: string | null): bigint | undefined {
  if (typeof value !== "string" || !/^\d+$/.test(value)) return undefined;
  try {
    return BigInt(value);
  } catch {
    return undefined;
  }
}

function randomSeedForRange(field: SeedFieldDefinition): string {
  const min = parseSeed(field.minValue);
  const max = parseSeed(field.maxValue);
  if (min === undefined || max === undefined || min > max) throw new Error("SEED_RANGE_UNAVAILABLE");
  const span = max - min + 1n;
  const bits = 64n;
  const capacity = 1n << bits;
  if (span > capacity) throw new Error("SEED_RANGE_TOO_LARGE");
  if (typeof globalThis.crypto !== "undefined" && typeof globalThis.crypto.getRandomValues === "function") {
    const raw = new Uint32Array(2);
    const limit = capacity - (capacity % span);
    let value = capacity;
    while (value >= limit) {
      globalThis.crypto.getRandomValues(raw);
      value = (BigInt(raw[0]) << 32n) | BigInt(raw[1]);
    }
    return (min + (value % span)).toString();
  }
  const fallback = BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER));
  return (min + (fallback % span)).toString();
}

function uniqueStrings(values: string[]): string[] {
  return [...new Set(values)];
}
