import type { DraftValue, GenerationValues, RecipeViewModel } from "../../types/generation";

export interface PromptVersion {
  id: string;
  promptId: string;
  version: number;
  text: string;
  createdAt: string;
}

export interface PromptRecord {
  id: string;
  projectId: string;
  name: string;
  tags: string[];
  versions: PromptVersion[];
}

export interface PromptVersionDiff {
  promptId: string;
  fromVersion: number;
  toVersion: number;
  addedLines: string[];
  removedLines: string[];
  changed: boolean;
}

export interface PromptStudioApplyResult {
  values?: GenerationValues;
  issue?: string;
}

export type PromptSnippetMode = "prepend" | "append" | "replace";

export function selectPromptTargetField(
  recipe: RecipeViewModel,
  requestedFieldKey?: string,
): { fieldKey?: string; issue?: string } {
  const fields = recipe.fields.filter((field) => field.type === "textarea");
  if (!fields.length) return { issue: "当前配方没有文字输入字段。" };
  if (requestedFieldKey) {
    return fields.some((field) => field.key === requestedFieldKey)
      ? { fieldKey: requestedFieldKey }
      : { issue: "所选文字输入字段不属于当前配方。" };
  }
  return fields.length === 1
    ? { fieldKey: fields[0].key }
    : { issue: "当前配方有多个文字输入字段，请明确选择目标字段。" };
}

export function normalizePromptText(text: string): string {
  return text.replace(/\r\n?/g, "\n").trim();
}

export function createPromptVersion(
  promptId: string,
  existing: readonly PromptVersion[],
  text: string,
  now: string,
  id: string,
): PromptVersion {
  return {
    id,
    promptId,
    version: Math.max(0, ...existing.map((item) => item.version)) + 1,
    text: normalizePromptText(text),
    createdAt: now,
  };
}

export function comparePromptVersions(left: PromptVersion, right: PromptVersion): PromptVersionDiff {
  const before = left.text.split("\n");
  const after = right.text.split("\n");
  let prefix = 0;
  while (prefix < before.length && prefix < after.length && before[prefix] === after[prefix]) prefix += 1;
  let suffix = 0;
  while (
    suffix < before.length - prefix &&
    suffix < after.length - prefix &&
    before[before.length - 1 - suffix] === after[after.length - 1 - suffix]
  ) suffix += 1;
  return {
    promptId: right.promptId,
    fromVersion: left.version,
    toVersion: right.version,
    removedLines: before.slice(prefix, before.length - suffix),
    addedLines: after.slice(prefix, after.length - suffix),
    changed: left.text !== right.text,
  };
}

export function promptVersionValues(versions: readonly PromptVersion[]): Array<Extract<DraftValue, { type: "string" }>> {
  return versions.map((version) => ({ type: "string", value: version.text }));
}

export function applyPromptVersionToStudio(
  recipe: RecipeViewModel,
  values: GenerationValues,
  fieldKey: string,
  version: PromptVersion,
): PromptStudioApplyResult {
  const field = recipe.fields.find((candidate) => candidate.key === fieldKey);
  if (!field || field.type !== "textarea") return { issue: "请选择当前配方的文字输入字段。" };
  return {
    values: {
      ...cloneValues(values),
      [field.key]: { type: "string", value: version.text },
    },
  };
}

export function applyPromptSnippetToStudio(
  recipe: RecipeViewModel,
  values: GenerationValues,
  fieldKey: string,
  text: string,
  mode: PromptSnippetMode,
): PromptStudioApplyResult {
  const field = recipe.fields.find((candidate) => candidate.key === fieldKey);
  if (!field || field.type !== "textarea") return { issue: "请选择当前配方的文字输入字段。" };
  const currentValue = values[field.key];
  const current = currentValue?.type === "string" ? currentValue.value : "";
  const next = mode === "replace"
    ? text
    : mode === "prepend"
      ? current ? `${text}\n${current}` : text
      : current ? `${current}\n${text}` : text;
  return {
    values: {
      ...cloneValues(values),
      [field.key]: { type: "string", value: next },
    },
  };
}

function cloneValues(values: GenerationValues): GenerationValues {
  return JSON.parse(JSON.stringify(values)) as GenerationValues;
}
