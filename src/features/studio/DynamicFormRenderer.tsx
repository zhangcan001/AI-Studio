import type { DraftValue, GenerationValues, RecipeField, RecipeViewModel } from "../../types/generation";
import { IntegerField } from "./fields/IntegerField";
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
}

export function DynamicFormRenderer({
  recipe,
  values,
  validationErrors,
  onChange,
  onGenerate,
  projectId,
  onImageAssetAvailabilityChange,
}: Props) {
  return (
    <div className="dynamic-form">
      {recipe.fields.map((field) =>
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
          field={field}
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
          field={field}
          value={value}
          error={error}
          onChange={(next) => onChange(field.key, next)}
        />
      );
    case "seed":
      return (
        <SeedField
          key={field.key}
          field={field}
          value={value}
          error={error}
          onChange={(next) => onChange(field.key, next)}
        />
      );
    case "image":
      return (
        <ImageField
          key={field.key}
          field={field}
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
          field={field}
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
          field={field}
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
          field={field}
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
          Unsupported Field: {fieldRecord.type}
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
        errors[field.key] = "This field is required.";
      }
    } else if (field.type === "integer") {
      if (!value || value.type !== "integer" || !Number.isInteger(value.value)) {
        if (field.required) errors[field.key] = "Enter a whole number.";
      } else if (field.min !== undefined && value.value < field.min) {
        errors[field.key] = `Must be at least ${field.min}.`;
      } else if (field.max !== undefined && value.value > field.max) {
        errors[field.key] = `Must be at most ${field.max}.`;
      }
    } else if (field.type === "seed" && value?.type === "seed_fixed") {
      if (!/^\d+$/.test(value.value) || value.value.length > 20) {
        errors[field.key] = "Use a decimal u64 seed string.";
      } else {
        try {
          const seed = BigInt(value.value);
          const min = BigInt(field.minValue ?? "0");
          const max = BigInt(field.maxValue ?? U64_MAX);
          if (seed < min || seed > max) {
            errors[field.key] = `Seed must be between ${min} and ${max}.`;
          }
        } catch {
          errors[field.key] = "Use a decimal u64 seed string.";
        }
      }
    } else if (
      field.type === "image" &&
      field.required &&
      (!value || value.type !== "image_asset" || !value.assetId.trim())
    ) {
      errors[field.key] = "Choose an image.";
    } else if (field.type === "images") {
      const imageIds = value?.type === "image_assets" ? value.assetIds : [];
      if (imageIds.length > field.maxItems || (imageIds.length > 0 && imageIds.length < field.minItems)) {
        errors[field.key] = `Choose between ${field.minItems} and ${field.maxItems} images.`;
      } else if (field.required && imageIds.length < field.minItems) {
        errors[field.key] = `Choose at least ${field.minItems} images.`;
      }
    } else if (field.type === "video" || field.type === "audio") {
      const expectedType = field.type === "video" ? "video_asset" : "audio_asset";
      if (field.required && (!value || value.type !== expectedType || !value.assetId.trim())) {
        errors[field.key] = field.type === "audio" ? "Choose an audio file." : "Choose a video.";
      }
    } else if (field.type === "videos" || field.type === "audios") {
      const expectedType = field.type === "videos" ? "video_assets" : "audio_assets";
      const assetIds = value?.type === expectedType ? value.assetIds : [];
      const label = field.type === "videos" ? "videos" : "audio files";
      if (assetIds.length > field.maxItems || (assetIds.length > 0 && assetIds.length < field.minItems)) {
        errors[field.key] = `Choose between ${field.minItems} and ${field.maxItems} ${label}.`;
      } else if (field.required && assetIds.length < field.minItems) {
        errors[field.key] = `Choose at least ${field.minItems} ${label}.`;
      }
    }
  }
  return errors;
}
