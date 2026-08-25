import type { DraftValue, GenerationValues, RecipeField, RecipeViewModel } from "../../types/generation";
import { fieldLabel } from "../../i18n/statusLabels";
import { IntegerField } from "./fields/IntegerField";
import { NumberField } from "./fields/NumberField";
import { ImageField } from "./fields/ImageField";
import { MultiImageField } from "./fields/MultiImageField";
import { MediaField } from "./fields/MediaField";
import { MultiMediaField } from "./fields/MultiMediaField";
import { SeedField } from "./fields/SeedField";
import { TextAreaField } from "./fields/TextAreaField";

const U64_MAX = "18446744073709551615";

interface Props {
  recipe: RecipeViewModel;
  values: GenerationValues;
  validationErrors: Record<string, string>;
  onChange: (key: string, value?: DraftValue) => void;
  onGenerate: () => void;
  projectId: string;
  onImageAssetAvailabilityChange?: (key: string, available: boolean) => void;
  hiddenFieldKeys?: string[];
}

export function DynamicFormRenderer({
  recipe,
  values,
  validationErrors,
  onChange,
  onGenerate,
  projectId,
  onImageAssetAvailabilityChange,
  hiddenFieldKeys = [],
}: Props) {
  return (
    <div className="dynamic-form">
      {recipe.fields.filter((field) => !hiddenFieldKeys.includes(field.key)).map((field) =>
        renderField(
          field,
          values,
          validationErrors,
          onChange,
          onGenerate,
          projectId,
          onImageAssetAvailabilityChange,
        ),
      )}
    </div>
  );
}

function renderField(
  field: RecipeField,
  values: GenerationValues,
  validationErrors: Record<string, string>,
  onChange: (key: string, value?: DraftValue) => void,
  onGenerate: () => void,
  projectId: string,
  onImageAssetAvailabilityChange?: (key: string, available: boolean) => void,
) {
  const value = values[field.key];
  const error = validationErrors[field.key];
  const fieldRecord = field as RecipeField & { type: string };
  switch (field.type) {
    case "textarea":
      return (
        <TextAreaField
          key={field.key}
          field={{ ...field, label: fieldLabel(field.key, field.label) }}
          value={value}
          error={error}
          onChange={(next) => onChange(field.key, next)}
          onGenerate={onGenerate}
        />
      );
    case "integer":
      return (
        <IntegerField
          key={field.key}
          field={{ ...field, label: fieldLabel(field.key, field.label) }}
          value={value}
          error={error}
          onChange={(next) => onChange(field.key, next)}
        />
      );
    case "number":
      return (
        <NumberField
          key={field.key}
          field={{ ...field, label: fieldLabel(field.key, field.label) }}
          value={value}
          error={error}
          onChange={(next) => onChange(field.key, next)}
        />
      );
    case "seed":
      return (
        <SeedField
          key={field.key}
          field={{ ...field, label: fieldLabel(field.key, field.label) }}
          value={value}
          error={error}
          onChange={(next) => onChange(field.key, next)}
        />
      );
    case "image":
      return (
        <ImageField
          key={field.key}
          field={{ ...field, label: fieldLabel(field.key, field.label) }}
          value={value}
          error={error}
          projectId={projectId}
          onChange={(next) => onChange(field.key, next)}
          onAvailabilityChange={(available) => onImageAssetAvailabilityChange?.(field.key, available)}
        />
      );
    case "images":
      return (
        <MultiImageField
          key={field.key}
          field={{ ...field, label: fieldLabel(field.key, field.label) }}
          value={value}
          error={error}
          projectId={projectId}
          onChange={(next) => onChange(field.key, next)}
          onAvailabilityChange={(available) => onImageAssetAvailabilityChange?.(field.key, available)}
        />
      );
    case "video":
    case "audio":
      return (
        <MediaField
          key={field.key}
          field={{ ...field, label: fieldLabel(field.key, field.label) }}
          value={value}
          error={error}
          projectId={projectId}
          onChange={(next) => onChange(field.key, next)}
          onAvailabilityChange={(available) => onImageAssetAvailabilityChange?.(field.key, available)}
        />
      );
    case "videos":
    case "audios":
      return (
        <MultiMediaField
          key={field.key}
          field={{ ...field, label: fieldLabel(field.key, field.label) }}
          value={value}
          error={error}
          projectId={projectId}
          onChange={(next) => onChange(field.key, next)}
          onAvailabilityChange={(available) => onImageAssetAvailabilityChange?.(field.key, available)}
        />
      );
    default:
      return (
        <div key={fieldRecord.key} className="unsupported-field">
          暂不支持的输入类型：{fieldRecord.type}
        </div>
      );
  }
}

export function validateRecipeValues(
  recipe: RecipeViewModel,
  values: GenerationValues,
): Record<string, string> {
  const errors: Record<string, string> = {};
  for (const field of recipe.fields) {
    const value = values[field.key];
    if (field.type === "textarea") {
      if (field.required && (!value || value.type !== "string" || value.value.trim() === "")) {
        errors[field.key] = "此项为必填项。";
      }
    } else if (field.type === "integer") {
      if (!value || value.type !== "integer" || !Number.isInteger(value.value)) {
        if (field.required) errors[field.key] = "请输入整数。";
      } else if (field.min !== undefined && value.value < field.min) {
        errors[field.key] = `数值不能小于 ${field.min}。`;
      } else if (field.max !== undefined && value.value > field.max) {
        errors[field.key] = `数值不能大于 ${field.max}。`;
      } else if (field.step !== undefined && field.step > 0 && value.value % field.step !== 0) {
        errors[field.key] = `数值必须是 ${field.step} 的倍数。`;
      }
    } else if (field.type === "number") {
      if (!value || value.type !== "number" || !Number.isFinite(value.value)) {
        if (field.required) errors[field.key] = "请输入数字。";
      } else if (field.min !== undefined && value.value < field.min) {
        errors[field.key] = `数值不能小于 ${field.min}。`;
      } else if (field.max !== undefined && value.value > field.max) {
        errors[field.key] = `数值不能大于 ${field.max}。`;
      } else if (
        field.step !== undefined
        && field.step > 0
        && !isNumberAlignedToStep(value.value, field.min ?? 0, field.step)
      ) {
        errors[field.key] = `数值必须按 ${field.step} 的步长输入。`;
      }
    } else if (field.type === "seed" && value?.type === "seed_fixed") {
      if (!/^\d+$/.test(value.value) || value.value.length > 20) {
        errors[field.key] = "请输入十进制随机种子。";
      } else {
        try {
          const seed = BigInt(value.value);
          const min = BigInt(field.minValue ?? "0");
          const max = BigInt(field.maxValue ?? U64_MAX);
          if (seed < min || seed > max) {
            errors[field.key] = `随机种子必须在 ${min} 到 ${max} 之间。`;
          }
        } catch {
          errors[field.key] = "请输入十进制随机种子。";
        }
      }
    } else if (
      field.type === "image" &&
      field.required &&
      (!value || value.type !== "image_asset" || !value.assetId.trim())
    ) {
      errors[field.key] = "请选择图片。";
    } else if (field.type === "images") {
      const imageIds = value?.type === "image_assets" ? value.assetIds : [];
      if (imageIds.length > field.maxItems || (imageIds.length > 0 && imageIds.length < field.minItems)) {
        errors[field.key] = `请选择 ${field.minItems} 到 ${field.maxItems} 张图片。`;
      } else if (field.required && imageIds.length < field.minItems) {
        errors[field.key] = `至少请选择 ${field.minItems} 张图片。`;
      }
    } else if (field.type === "video" || field.type === "audio") {
      const expectedType = field.type === "video" ? "video_asset" : "audio_asset";
      if (field.required && (!value || value.type !== expectedType || !value.assetId.trim())) {
        errors[field.key] = field.type === "audio" ? "请选择音频文件。" : "请选择视频。";
      }
    } else if (field.type === "videos" || field.type === "audios") {
      const expectedType = field.type === "videos" ? "video_assets" : "audio_assets";
      const assetIds = value?.type === expectedType ? value.assetIds : [];
      const label = field.type === "videos" ? "视频" : "音频文件";
      if (assetIds.length > field.maxItems || (assetIds.length > 0 && assetIds.length < field.minItems)) {
        errors[field.key] = `请选择 ${field.minItems} 到 ${field.maxItems} 个${label}。`;
      } else if (field.required && assetIds.length < field.minItems) {
        errors[field.key] = `至少请选择 ${field.minItems} 个${label}。`;
      }
    }
  }
  return errors;
}

export function isNumberAlignedToStep(value: number, base: number, step: number): boolean {
  if (!Number.isFinite(value) || !Number.isFinite(base) || !Number.isFinite(step) || step <= 0) return false;
  const quotient = (value - base) / step;
  return Math.abs(quotient - Math.round(quotient)) <= 1e-9 * Math.max(1, Math.abs(quotient));
}
