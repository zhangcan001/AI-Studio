import type { DraftValue } from "../../../types/generation";

interface Props {
  field: {
    key: string;
    label: string;
    minValue?: string | null;
    maxValue?: string | null;
  };
  value?: DraftValue;
  error?: string;
  onChange: (value: DraftValue) => void;
}

export function SeedField({ field, value, error, onChange }: Props) {
  const mode = value?.type === "seed_fixed" ? "fixed" : "random";
  const fixedValue = value?.type === "seed_fixed" ? value.value : "";
  const minValue = field.minValue ?? "0";
  const maxValue = field.maxValue ?? "18446744073709551615";
  return (
    <label className="field-control">
      <span>{field.label}</span>
      <select
        value={mode}
        onChange={(event) => {
          onChange(
            event.target.value === "fixed"
              ? { type: "seed_fixed", value: fixedValue }
              : { type: "seed_random" },
          );
        }}
      >
        <option value="random">随机</option>
        <option value="fixed">固定</option>
      </select>
      {mode === "fixed" && (
        <input
          type="text"
          inputMode="numeric"
          value={fixedValue}
          placeholder="请输入十进制随机种子"
          onChange={(event) => onChange({ type: "seed_fixed", value: event.target.value })}
          aria-invalid={Boolean(error)}
        />
      )}
      <small className="field-hint">
        固定种子使用十进制字符串。范围：{minValue} – {maxValue}
      </small>
      {error && <small className="field-error">{error}</small>}
    </label>
  );
}
