import type { DraftValue } from "../../../types/generation";

interface Props {
  field: {
    key: string;
    label: string;
    required: boolean;
  };
  value?: DraftValue;
  error?: string;
  onChange: (value: DraftValue) => void;
  onGenerate: () => void;
}

export function TextAreaField({ field, value, error, onChange, onGenerate }: Props) {
  const text = value?.type === "string" ? value.value : "";
  return (
    <label className="field-control">
      <span>
        {field.label}
        {field.required && <em>必填</em>}
      </span>
      <textarea
        value={text}
        rows={4}
        onChange={(event) => onChange({ type: "string", value: event.target.value })}
        onKeyDown={(event) => {
          if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
            event.preventDefault();
            onGenerate();
          }
        }}
        aria-invalid={Boolean(error)}
      />
      {error && <small className="field-error">{error}</small>}
    </label>
  );
}
