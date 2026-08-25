import { useState } from "react";
import { PROMPT_TEMPLATE_VARIABLE_GROUPS, promptTemplateVariableLabel } from "./promptTemplateState";

interface Props {
  compact?: boolean;
}

export function PromptTemplateVariableHelper({ compact = false }: Props) {
  const [copied, setCopied] = useState<string>();

  async function copy(variable: string) {
    try {
      await navigator.clipboard?.writeText(promptTemplateVariableLabel(variable));
      setCopied(variable);
      window.setTimeout(() => setCopied((current) => current === variable ? undefined : current), 1200);
    } catch {
      setCopied(undefined);
    }
  }

  return (
    <details className={`prompt-template-helper${compact ? " prompt-template-helper-compact" : ""}`}>
      <summary>模板变量帮助</summary>
      <p>提示词正文中使用 <code>{"{{variable.path}}"}</code>。点击变量可复制；片段仍按普通文本处理。</p>
      <div className="prompt-template-variable-groups">
        {PROMPT_TEMPLATE_VARIABLE_GROUPS.map((group) => (
          <div key={group.label} className="prompt-template-variable-group">
            <strong>{group.label}</strong>
            <div>
              {group.variables.map((variable) => (
                <button type="button" key={variable} className="prompt-template-variable-chip" onClick={() => void copy(variable)} title="复制变量">
                  {promptTemplateVariableLabel(variable)}{copied === variable && <small>已复制</small>}
                </button>
              ))}
            </div>
          </div>
        ))}
      </div>
    </details>
  );
}
