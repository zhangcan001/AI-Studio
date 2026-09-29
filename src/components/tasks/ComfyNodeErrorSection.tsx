import { useEffect, useRef, useState } from "react";
import type { TaskNodeError } from "../../types/history";

export function ComfyNodeErrorSection({
  nodeErrors,
  rawError,
  title = "ComfyUI 节点校验详情",
}: {
  nodeErrors: TaskNodeError[];
  rawError: unknown;
  title?: string;
}) {
  const rawText = rawError === undefined ? "" : JSON.stringify(rawError, null, 2) ?? "";
  const [copied, setCopied] = useState(false);
  const copyTimer = useRef<number | undefined>(undefined);

  useEffect(() => () => {
    if (copyTimer.current !== undefined) window.clearTimeout(copyTimer.current);
  }, []);

  async function copyRawError() {
    if (!rawText || !navigator.clipboard) return;
    await navigator.clipboard.writeText(rawText);
    setCopied(true);
    copyTimer.current = window.setTimeout(() => setCopied(false), 1600);
  }

  return (
    <section className="detail-section comfy-node-error-section" aria-label={title}>
      <div className="section-heading">
        <div>
          <span className="section-label">ComfyUI 校验详情</span>
          <h3>{title}</h3>
        </div>
      </div>
      {nodeErrors.length > 0 ? (
        <div className="comfy-node-error-list">
          {nodeErrors.map((error, index) => (
            <article className="comfy-node-error" key={`${error.nodeId}-${index}`}>
              <div className="comfy-node-error-heading">
                <strong>节点 {error.nodeId}</strong>
                {error.nodeType && <code>{error.nodeType}</code>}
              </div>
              <dl>
                {error.input && <ErrorFact label="输入" value={error.input} />}
                {error.errorType && <ErrorFact label="错误类型" value={error.errorType} />}
                <ErrorFact label="消息" value={error.message} />
                {error.details && <ErrorFact label="详情" value={error.details} />}
                {error.receivedValue !== undefined && <ErrorFact label="当前值" value={formatDiagnosticValue(error.receivedValue)} code />}
                {error.expectedConfig !== undefined && <ErrorFact label="期望配置" value={formatDiagnosticValue(error.expectedConfig)} code />}
              </dl>
            </article>
          ))}
        </div>
      ) : (
        <p className="disabled-note">ComfyUI 未返回结构化 node_errors；请查看原始错误消息。</p>
      )}
      {rawText && (
        <details className="comfy-raw-error">
          <summary>展开原始 JSON</summary>
          <div className="comfy-raw-error-actions">
            <button type="button" className="quiet-button" onClick={() => void copyRawError()}>
              {copied ? "已复制" : "复制"}
            </button>
          </div>
          <pre>{rawText}</pre>
        </details>
      )}
    </section>
  );
}

function ErrorFact({ label, value, code = false }: { label: string; value: string; code?: boolean }) {
  return <div><dt>{label}</dt><dd className={code ? "diagnostic-code" : undefined}>{value}</dd></div>;
}

function formatDiagnosticValue(value: unknown): string {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}
