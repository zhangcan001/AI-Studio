import { useMemo } from "react";
import type { RecipeField } from "../../types/generation";
import { validateResolution } from "./resolution";
import type { ResolutionPreset } from "./resolutionPresets";

type IntegerRecipeField = Extract<RecipeField, { type: "integer" }>;

interface Props {
  widthField: IntegerRecipeField;
  heightField: IntegerRecipeField;
  width: number | undefined;
  height: number | undefined;
  presets: ResolutionPreset[];
  presetsOnly?: boolean;
  disabled: boolean;
  onChange: (next: { width?: number; height?: number }) => void;
}

export function ResolutionControl({
  widthField,
  heightField,
  width,
  height,
  presets,
  presetsOnly = false,
  disabled,
  onChange,
}: Props) {
  const selectedPreset = useMemo(
    () => presets.find((preset) => preset.width === width && preset.height === height),
    [height, presets, width],
  );
  const validation = validateResolution(
    {
      workflowId: "resolution-control",
      workflowVersionId: "resolution-control",
      recipeId: "resolution-control",
      name: "resolution-control",
      category: "",
      mode: "",
      fields: [widthField, heightField],
    },
    width,
    height,
  );
  const widthError = validation.errors.width;
  const heightError = validation.errors.height;

  return (
    <section className="resolution-control" aria-labelledby="resolution-control-title">
      <div className="resolution-control-heading">
        <div>
          <span className="section-label">输出设置</span>
          <h3 id="resolution-control-title">分辨率</h3>
        </div>
        <span className="resolution-control-current">
          当前输出：{width ?? "—"} × {height ?? "—"}
        </span>
      </div>
      <div className="resolution-control-grid">
        <label className="resolution-control-preset">
          <span>常用预设</span>
          <select
            value={selectedPreset?.id ?? (presetsOnly ? "unsupported" : "custom")}
            onChange={(event) => {
              const preset = presets.find((item) => item.id === event.target.value);
              if (preset) onChange({ width: preset.width, height: preset.height });
            }}
            disabled={disabled}
          >
            {presets.length === 0 && <option value={presetsOnly ? "unsupported" : "custom"}>当前配方无可用预设</option>}
            {presetsOnly && !selectedPreset && presets.length > 0 && (
              <option value="unsupported" disabled>
                当前输出不在图片规格范围内
              </option>
            )}
            {presets.map((preset) => (
              <option key={preset.id} value={preset.id}>
                {preset.label} · {preset.width} × {preset.height}{preset.tier === "2k" ? " · 2K" : ""}
              </option>
            ))}
            {!presetsOnly && <option value="custom">自定义</option>}
          </select>
        </label>
        {!presetsOnly && (
          <div className="resolution-control-custom" aria-label="自定义分辨率">
            <label>
              <span>宽度</span>
              <input
                type="number"
                inputMode="numeric"
                min={widthField.min}
                max={widthField.max}
                step={widthField.step ?? 1}
                value={width ?? ""}
                onChange={(event) => onChange({ width: numberOrUndefined(event.target.value), height })}
                disabled={disabled}
                aria-invalid={Boolean(widthError)}
              />
              {widthError && <small className="field-error">{widthError}</small>}
            </label>
            <span className="resolution-control-times" aria-hidden="true">×</span>
            <label>
              <span>高度</span>
              <input
                type="number"
                inputMode="numeric"
                min={heightField.min}
                max={heightField.max}
                step={heightField.step ?? 1}
                value={height ?? ""}
                onChange={(event) => onChange({ width, height: numberOrUndefined(event.target.value) })}
                disabled={disabled}
                aria-invalid={Boolean(heightError)}
              />
              {heightError && <small className="field-error">{heightError}</small>}
            </label>
          </div>
        )}
      </div>
      <small className="field-hint">
        {presetsOnly
          ? "仅支持图片规格中的 14 档 16:9 输出分辨率。"
          : `合法范围：宽度 ${widthField.min ?? 1}–${widthField.max ?? "不限"}，高度 ${heightField.min ?? 1}–${heightField.max ?? "不限"}；不符合配方步长的值不会自动调整。`}
      </small>
    </section>
  );
}

function numberOrUndefined(value: string): number | undefined {
  if (value.trim() === "") return undefined;
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : undefined;
}
