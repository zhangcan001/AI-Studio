import type {
  WorkflowFieldType,
  WorkflowInputMappingView,
  WorkflowInputView,
  WorkflowOnboardingDraftView,
} from "../../types/workflowOnboarding";

export const parameterFieldTypes: WorkflowFieldType[] = [
  "textarea",
  "integer",
  "number",
  "seed",
  "image",
  "images",
  "video",
  "videos",
  "audio",
  "audios",
];

export interface MappingDraft {
  semanticKey: string;
  fieldType: WorkflowFieldType;
  label: string;
  required: boolean;
  defaultValue: string;
  minValue: string;
  maxValue: string;
  minItems: string;
  maxItems: string;
  itemIndex: string;
  step: string;
}

export type ParameterMappingEdit = {
  mapping: WorkflowInputMappingView;
  draft: MappingDraft;
};

export function mappingToDraft(mapping: WorkflowInputMappingView): MappingDraft {
  return {
    semanticKey: mapping.semanticKey,
    fieldType: mapping.fieldType,
    label: mapping.label,
    required: mapping.required,
    defaultValue: mapping.defaultValue ?? "",
    minValue: mapping.minValue ?? "",
    maxValue: mapping.maxValue ?? "",
    minItems: mapping.minItems?.toString() ?? "",
    maxItems: mapping.maxItems?.toString() ?? "",
    itemIndex: mapping.itemIndex?.toString() ?? "",
    step: mapping.step ?? "",
  };
}

export function defaultMapping(nodeId: string, input: WorkflowInputView): MappingDraft {
  const safeName = input.name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "") || "value";
  const fieldType = supportedParameterFieldType(input) ?? "textarea";
  return {
    semanticKey: input.suggestedSemanticKey ?? `input_${nodeId}_${safeName}`,
    fieldType,
    label: fieldLabel(input.name),
    required: true,
    defaultValue: !input.isLinked && (fieldType === "textarea" || fieldType === "integer" || fieldType === "number" || fieldType === "seed")
      ? input.currentValueSummary === "random" ? "" : input.currentValueSummary
      : "",
    minValue: input.numericMin ?? "",
    maxValue: input.numericMax ?? "",
    minItems: "",
    maxItems: "",
    itemIndex: "",
    step: input.numericStep ?? "",
  };
}

export function emptyMapping(): MappingDraft {
  return {
    semanticKey: "input_value",
    fieldType: "textarea",
    label: "值",
    required: true,
    defaultValue: "",
    minValue: "",
    maxValue: "",
    minItems: "",
    maxItems: "",
    itemIndex: "",
    step: "",
  };
}

export function supportedParameterFieldType(input: WorkflowInputView): WorkflowFieldType | undefined {
  if (input.suggestedType && parameterFieldTypes.includes(input.suggestedType as WorkflowFieldType)) {
    return input.suggestedType as WorkflowFieldType;
  }
  return undefined;
}

export function isExposableWorkflowInput(input: WorkflowInputView): boolean {
  const linkedSemantic = input.suggestedSemanticKey?.toLowerCase();
  const graphSemantic = [
    "prompt", "negative_prompt", "width", "height", "duration_seconds", "seed",
    "reference_image", "reference_video", "reference_audio",
  ].includes(linkedSemantic ?? "");
  return (input.bindable || (input.isLinked && graphSemantic))
    && Boolean(supportedParameterFieldType(input))
    && !isDangerousParameterName(input.name);
}

export function isDangerousParameterName(name: string): boolean {
  const lower = name.toLowerCase();
  return [
    "model_path", "filename_prefix", "output_directory", "output_dir", "filesystem_path", "file_path",
    "custom_python", "python_path", "backend_endpoint", "endpoint", "filename", "directory", "folder",
    "path", "prefix", "python", "device", "provider", "checkpoint", "ckpt", "unet", "vae", "clip", "lora", "model",
  ].some((token) => lower === token || lower.includes(token));
}

export function optionalText(value: string): string | undefined {
  return value.trim() || undefined;
}

export function optionalNumber(value: string): number | undefined {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : undefined;
}

export function localizeWorkflowIssue(value: string): string {
  const normalized = value.toLowerCase();
  if (normalized.includes("api") && normalized.includes("format")) return "该文件不是 ComfyUI API 格式工作流。";
  if (normalized.includes("recipe")) return "配方校验未通过，请检查输入映射和输出映射。";
  if (normalized.includes("binding")) return "输入绑定校验未通过，请检查每个输入映射。";
  if (normalized.includes("output")) return "输出校验未通过，请至少配置一个有效输出。";
  if (normalized.includes("manifest")) return "工作流基本信息校验未通过。";
  if (normalized.includes("capability")) return "ComfyUI 兼容性校验未通过。";
  if (normalized.includes("dry run")) return "工作流试运行未通过。";
  return "工作流校验未通过，请查看技术详情。";
}

export function mappingKey(nodeId: string, inputName: string): string {
  return `${nodeId}:${inputName}`;
}

export function fieldLabel(value: string): string {
  return value
    .split(/[_-]+/g)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

export function parameterMappingDraftKey(mapping: WorkflowOnboardingDraftView["inputMappings"][number]): string {
  return mappingKey(mapping.targetNode, mapping.targetInput);
}
