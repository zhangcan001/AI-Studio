import type { DraftValue } from "../../../types/generation";

interface Props {
  field: {
    key: string;
    label: string;
    required: boolean;
    min?: number;
    max?: number;
    default?: number;
  };
  value?: DraftValue;
  error?: string;
  onChange: (value?: DraftValue) => void;
}

export function IntegerField({ field, value, error, onChange }: Props) {
  const integer = value?.type === "integer" ? value.value : field.default ?? "";
  return (
    <label className="field-control">
      <span>
        {field.label}
        {field.required && <em>必填</em>}
      </span>
      <input
        type="number"
        value={integer}
        min={field.min}
        max={field.max}
        step={1}
        onChange={(event) => {
          if (event.target.value === "") {
            onChange(undefined);
            return;
          }
          onChange({ type: "integer", value: Number(event.target.value) });
        }}
        aria-invalid={Boolean(error)}
      />
      <small className="field-hint">
        {field.min !== undefined && field.max !== undefined
          ? `${field.min} – ${field.max}`
          : "请输入整数"}
      </small>
      {error && <small className="field-error">{error}</small>}
    </label>
  );
}
