import type { DraftValue } from "../../../types/generation";

interface Props {
  field: {
    key: string;
    label: string;
    required: boolean;
    min?: number;
    max?: number;
    step?: number;
    default?: number;
  };
  value?: DraftValue;
  error?: string;
  onChange: (value?: DraftValue) => void;
}

export function NumberField({ field, value, error, onChange }: Props) {
  const number = value?.type === "number" ? value.value : field.default ?? "";
  return (
    <label className="field-control">
      <span>
        {field.label}
        {field.required && <em>必填</em>}
      </span>
      <input
        type="number"
        inputMode="decimal"
        value={number}
        min={field.min}
        max={field.max}
        step={field.step ?? "any"}
        onChange={(event) => {
          if (event.target.value === "") {
            onChange(undefined);
            return;
          }
          onChange({ type: "number", value: Number(event.target.value) });
        }}
        aria-invalid={Boolean(error)}
      />
      <small className="field-hint">
        {field.min !== undefined || field.max !== undefined
          ? `${field.min ?? "−∞"} – ${field.max ?? "∞"}${field.step !== undefined ? ` · 步长 ${field.step}` : ""}`
          : "请输入数字"}
      </small>
      {error && <small className="field-error">{error}</small>}
    </label>
  );
}
