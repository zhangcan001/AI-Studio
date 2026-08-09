import { formatUiError } from "./errorMessages";

interface Props {
  error: unknown;
  className?: string;
}

export function UiErrorNotice({ error, className = "error-message" }: Props) {
  const formatted = formatUiError(error);
  const hasTechnicalDetails = Boolean(
    formatted.technicalMessage && formatted.technicalMessage !== formatted.message,
  );
  return (
    <div className={className} role="alert">
      <p>{formatted.message}</p>
      {hasTechnicalDetails && (
        <details className="technical-error-details">
          <summary>技术详情</summary>
          {formatted.code && <p>错误代码：<code>{formatted.code}</code></p>}
          <p>原始信息：<code>{formatted.technicalMessage}</code></p>
        </details>
      )}
    </div>
  );
}
