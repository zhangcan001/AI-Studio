import type { DraftValue, GenerationValues, RecipeViewModel } from "../../types/generation";

export type RuntimeKind = "image" | "video" | "audio" | "mixed";
export type RuntimeFilter = "all" | RuntimeKind;

export function runtimeKindFor(recipe: Pick<RecipeViewModel, "category" | "mode">): RuntimeKind {
  const source = `${recipe.category} ${recipe.mode}`.toLocaleLowerCase();
  if (/(video|movie|animation|视频)/.test(source)) return "video";
  if (/(audio|sound|music|音频|声音)/.test(source)) return "audio";
  if (/(image|photo|picture|图像|图片)/.test(source)) return "image";
  return "mixed";
}

export function runtimeKindLabel(kind: RuntimeKind): string {
  switch (kind) {
    case "image":
      return "图片";
    case "video":
      return "视频";
    case "audio":
      return "音频";
    case "mixed":
      return "复合";
  }
}

export function filterRuntimeCatalog(
  catalog: RecipeViewModel[],
  filter: RuntimeFilter,
  search = "",
): RecipeViewModel[] {
  const needle = search.trim().toLocaleLowerCase();
  return catalog.filter((recipe) => {
    const kindMatches = filter === "all" || runtimeKindFor(recipe) === filter;
    const searchMatches = !needle || `${recipe.name} ${recipe.category} ${recipe.mode}`.toLocaleLowerCase().includes(needle);
    return kindMatches && searchMatches;
  });
}

export type RuntimeParameterValues = Record<string, number>;

export function sanitizeRuntimeParameterValues(input: Record<string, unknown>): RuntimeParameterValues {
  return Object.fromEntries(
    Object.entries(input).flatMap(([key, value]) => (
      key.trim() && typeof value === "number" && Number.isSafeInteger(value)
        ? [[key.trim(), value]]
        : []
    )),
  );
}

export interface RuntimeParameterProfile {
  id: string;
  workflowVersionId: string;
  recipeId: string;
  name: string;
  values: RuntimeParameterValues;
  updatedAt: string;
}

const profileStorageKey = "ai-studio.runtime-parameter-profiles.v1";

export function runtimeProfileKey(recipe: Pick<RecipeViewModel, "workflowVersionId" | "recipeId">): string {
  return `${recipe.workflowVersionId}:${recipe.recipeId}`;
}

export interface LegacyRuntimeParameterProfile extends RuntimeParameterProfile {
  values: RuntimeParameterValues;
}

export function listLegacyRuntimeParameterProfiles(): LegacyRuntimeParameterProfile[] {
  if (typeof globalThis.localStorage === "undefined") return [];
  try {
    const raw = globalThis.localStorage.getItem(profileStorageKey);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.flatMap((item) => {
      if (!isRecord(item)) return [];
      if (typeof item.id !== "string" || typeof item.workflowVersionId !== "string" || typeof item.recipeId !== "string" || typeof item.name !== "string" || typeof item.updatedAt !== "string" || !isRecord(item.values)) return [];
      return [{
        id: item.id,
        workflowVersionId: item.workflowVersionId,
        recipeId: item.recipeId,
        name: item.name,
        values: sanitizeRuntimeParameterValues(item.values),
        updatedAt: item.updatedAt,
      }];
    });
  } catch {
    return [];
  }
}

export function removeLegacyRuntimeParameterProfiles(): void {
  if (typeof globalThis.localStorage === "undefined") return;
  globalThis.localStorage.removeItem(profileStorageKey);
}

export interface RuntimeProfileMigrationResult {
  profile?: RuntimeParameterProfile;
  unresolvedKeys: string[];
}

const legacyFieldAliases: Record<string, string[]> = {
  steps: ["steps"],
  width: ["width"],
  height: ["height"],
  durationSeconds: ["durationSeconds", "duration_seconds"],
};

export function migrateLegacyRuntimeProfile(
  recipe: RecipeViewModel,
  profile: LegacyRuntimeParameterProfile,
): RuntimeProfileMigrationResult {
  const integerKeys = new Set(recipe.fields.filter((field) => field.type === "integer").map((field) => field.key));
  const values: RuntimeParameterValues = {};
  const unresolvedKeys: string[] = [];
  for (const [legacyKey, value] of Object.entries(profile.values)) {
    // Concurrency was never an executable setting; deliberately omit it from
    // the backend profile instead of carrying forward misleading state.
    if (legacyKey === "concurrency") continue;
    const targetKey = (legacyFieldAliases[legacyKey] ?? [legacyKey]).find((candidate) => integerKeys.has(candidate));
    if (!targetKey) {
      unresolvedKeys.push(legacyKey);
      continue;
    }
    values[targetKey] = value;
  }
  if (unresolvedKeys.length) return { unresolvedKeys };
  return {
    unresolvedKeys: [],
    profile: {
      ...profile,
      values,
    },
  };
}

export interface AppliedRuntimeProfile {
  values: GenerationValues;
  appliedFields: string[];
  ignoredParameters: string[];
}

export function applyRuntimeParameterProfile(
  recipe: RecipeViewModel,
  values: GenerationValues,
  profile: RuntimeParameterProfile,
): AppliedRuntimeProfile {
  const nextValues = { ...values };
  const appliedFields: string[] = [];
  const ignoredParameters: string[] = [];

  for (const [fieldKey, value] of Object.entries(profile.values)) {
    const field = recipe.fields.find((candidate) => candidate.type === "integer" && candidate.key === fieldKey);
    if (!field || field.type !== "integer") {
      ignoredParameters.push(fieldKey);
      continue;
    }
    const bounded = Math.min(field.max ?? Number.MAX_SAFE_INTEGER, Math.max(field.min ?? Number.MIN_SAFE_INTEGER, value));
    nextValues[field.key] = { type: "integer", value: Math.round(bounded) } satisfies DraftValue;
    appliedFields.push(field.key);
  }

  return { values: nextValues, appliedFields, ignoredParameters };
}

export type WorkflowImportFormat = "api" | "ui" | "unknown";

export interface WorkflowImportQualityReport {
  accepted: boolean;
  format: WorkflowImportFormat;
  nodeCount: number;
  uniqueClassCount: number;
  outputCandidateCount: number;
  errors: string[];
  warnings: string[];
}

const maxWorkflowTextLength = 20 * 1024 * 1024;

export function inspectWorkflowImport(text: string, fileName = "workflow.json"): WorkflowImportQualityReport {
  const errors: string[] = [];
  const warnings: string[] = [];
  if (!fileName.toLocaleLowerCase().endsWith(".json")) errors.push("工作流文件必须是 JSON。");
  if (text.length > maxWorkflowTextLength) errors.push("工作流 JSON 超过 20 MB 限制。");

  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return { accepted: false, format: "unknown", nodeCount: 0, uniqueClassCount: 0, outputCandidateCount: 0, errors: [...errors, "文件不是有效的 JSON。"], warnings };
  }

  if (!isRecord(parsed)) {
    return { accepted: false, format: "unknown", nodeCount: 0, uniqueClassCount: 0, outputCandidateCount: 0, errors: [...errors, "工作流根节点必须是对象。"], warnings };
  }

  if (Array.isArray(parsed.nodes)) {
    const nodes = parsed.nodes.filter(isRecord);
    const classes = new Set(nodes.map((node) => stringValue(node.type) ?? stringValue(node.class_type)).filter((value): value is string => Boolean(value)));
    errors.push("检测到 ComfyUI 编辑器格式。请在 ComfyUI 中导出 API 格式工作流后重新导入。");
    return { accepted: false, format: "ui", nodeCount: nodes.length, uniqueClassCount: classes.size, outputCandidateCount: countOutputCandidates(classes), errors, warnings };
  }

  const entries = Object.entries(parsed);
  const nodes: Array<[string, Record<string, unknown>]> = [];
  for (const [nodeId, value] of entries) {
    if (isRecord(value) && typeof value.class_type === "string") nodes.push([nodeId, value]);
  }
  if (!nodes.length) {
    return { accepted: false, format: "unknown", nodeCount: 0, uniqueClassCount: 0, outputCandidateCount: 0, errors: [...errors, "未识别为 ComfyUI API 工作流。"], warnings };
  }

  const classes = new Set(nodes.map(([, value]) => (value as Record<string, unknown>).class_type).filter((value): value is string => typeof value === "string"));
  for (const [nodeId, value] of nodes) {
    if (!isRecord(value.inputs)) warnings.push(`节点 ${nodeId} 缺少 inputs 对象，导入后需要复核输入映射。`);
  }
  if (containsAbsolutePath(parsed)) warnings.push("检测到本机绝对路径；分享或迁移前请确认路径不会泄露本机信息。");
  if (containsSecretLikeKey(parsed)) errors.push("工作流包含疑似凭据字段，请移除凭据后再导入。");
  const outputCandidateCount = countOutputCandidates(classes);
  if (!outputCandidateCount) warnings.push("未发现明显的输出节点；发布前必须完成输出映射。");

  return {
    accepted: errors.length === 0,
    format: "api",
    nodeCount: nodes.length,
    uniqueClassCount: classes.size,
    outputCandidateCount,
    errors,
    warnings,
  };
}

function countOutputCandidates(classes: Set<string>): number {
  return [...classes].filter((value) => /(save|preview|output|export|video|image)/i.test(value)).length;
}

function containsAbsolutePath(value: unknown): boolean {
  if (typeof value === "string") return /^(?:[a-z]:[\\/]|\\\\|\/)(?:[^/]|$)/i.test(value);
  if (Array.isArray(value)) return value.some(containsAbsolutePath);
  if (isRecord(value)) return Object.values(value).some(containsAbsolutePath);
  return false;
}

function containsSecretLikeKey(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(containsSecretLikeKey);
  if (!isRecord(value)) return false;
  const secretKeys = new Set([
    "password",
    "passwd",
    "secret",
    "token",
    "access_token",
    "refresh_token",
    "api_key",
    "apikey",
    "authorization",
    "credential",
    "credentials",
  ]);
  return Object.entries(value).some(([key, child]) => {
    const normalized = key
      .trim()
      .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
      .replace(/[\s-]+/g, "_")
      .toLocaleLowerCase();
    return secretKeys.has(normalized) || secretKeys.has(normalized.replace(/_/g, "")) || containsSecretLikeKey(child);
  });
}

export interface RuntimeQueueItem {
  id: string;
  runtimeId: string;
}

export interface RuntimeQueueState {
  id: string;
  enabled: boolean;
  readiness: string;
  capability?: string;
}

export interface RuntimeQueueValidationIssue {
  code: string;
  message: string;
  itemId?: string;
}

export interface RuntimeQueueValidationResult {
  valid: boolean;
  runtimeIds: string[];
  issues: RuntimeQueueValidationIssue[];
}

export function validateMultiRuntimeQueue(
  items: RuntimeQueueItem[],
  runtimes: RuntimeQueueState[],
  options: { requireMultipleRuntimes?: boolean } = {},
): RuntimeQueueValidationResult {
  const issues: RuntimeQueueValidationIssue[] = [];
  const runtimeMap = new Map(runtimes.map((runtime) => [runtime.id, runtime]));
  const runtimeIds = [...new Set(items.map((item) => item.runtimeId))];
  const seenItems = new Set<string>();

  if (!items.length) issues.push({ code: "QUEUE_EMPTY", message: "队列至少需要一个任务。" });
  if (items.length > 100) issues.push({ code: "QUEUE_TOO_LARGE", message: "队列最多支持 100 个任务。" });
  if (options.requireMultipleRuntimes && runtimeIds.length < 2) issues.push({ code: "QUEUE_NOT_MULTI_RUNTIME", message: "该校验要求队列至少覆盖两个运行时。" });

  for (const item of items) {
    if (seenItems.has(item.id)) issues.push({ code: "QUEUE_DUPLICATE_ITEM", message: "队列中存在重复任务。", itemId: item.id });
    seenItems.add(item.id);
    const runtime = runtimeMap.get(item.runtimeId);
    if (!runtime) {
      issues.push({ code: "QUEUE_RUNTIME_UNAVAILABLE", message: "任务引用的运行时不可用。", itemId: item.id });
      continue;
    }
    if (!runtime.enabled) issues.push({ code: "QUEUE_RUNTIME_DISABLED", message: "任务引用的运行时已停用。", itemId: item.id });
    if (runtime.readiness.toLocaleUpperCase() !== "READY") issues.push({ code: "QUEUE_RUNTIME_NOT_READY", message: "任务引用的运行时尚未达到 READY。", itemId: item.id });
    if (runtime.capability && runtime.capability.toLocaleUpperCase() !== "READY") issues.push({ code: "QUEUE_RUNTIME_CAPABILITY_INVALID", message: "任务引用的运行时能力检查未通过。", itemId: item.id });
  }

  return { valid: issues.length === 0, runtimeIds, issues };
}

export type RuntimeGateStatus = "PASS" | "BLOCKED" | "ENVIRONMENT_BLOCKED";

export interface RuntimeGateInput {
  packageImported: boolean;
  capabilityReady: boolean;
  quickTestPassed: boolean;
  outputVerified: boolean;
  historyAvailable: boolean;
  environmentInputAvailable?: boolean;
  environmentBlockCode?: string;
}

export interface RuntimeGateResult {
  status: RuntimeGateStatus;
  issues: string[];
}

export function evaluateRuntimeGate(input: RuntimeGateInput): RuntimeGateResult {
  if (input.environmentInputAvailable === false) {
    return { status: "ENVIRONMENT_BLOCKED", issues: [input.environmentBlockCode ?? "REQUIRED_RUNTIME_INPUT_MISSING"] };
  }
  const issues: string[] = [];
  if (!input.packageImported) issues.push("RUNTIME_PACKAGE_MISSING");
  if (!input.capabilityReady) issues.push("RUNTIME_CAPABILITY_NOT_READY");
  if (!input.quickTestPassed) issues.push("RUNTIME_QUICK_TEST_FAILED");
  if (!input.outputVerified) issues.push("RUNTIME_OUTPUT_NOT_VERIFIED");
  if (!input.historyAvailable) issues.push("RUNTIME_HISTORY_NOT_VERIFIED");
  return { status: issues.length ? "BLOCKED" : "PASS", issues };
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim() ? value : undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
