import type { DraftValue } from "../../../types/generation";

interface Props {
  field: { key: string; label: string };
  value?: DraftValue;
  error?: string;
  onChange: (value: DraftValue) => void;
}

export function SeedField({ field, value, error, onChange }: Props) {
  const mode = value?.type === "seed_fixed" ? "fixed" : "random";
  const fixedValue = value?.type === "seed_fixed" ? value.value : "";
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
        <option value="random">Random</option>
        <option value="fixed">Fixed</option>
      </select>
      {mode === "fixed" && (
        <input
          type="text"
          inputMode="numeric"
          value={fixedValue}
          placeholder="Decimal seed"
          onChange={(event) => onChange({ type: "seed_fixed", value: event.target.value })}
          aria-invalid={Boolean(error)}
        />
      )}
      <small className="field-hint">Fixed seeds use a decimal string.</small>
      {error && <small className="field-error">{error}</small>}
    </label>
  );
}
