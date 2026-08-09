interface Props {
  error?: string | null;
  onRetry: () => void;
}

export function StartupScreen({ error, onRetry }: Props) {
  return (
    <main className="startup-screen" role="status" aria-live="polite">
      <div className="startup-card">
        <span className="section-label">AI Studio</span>
        <h1>{error ? "创作环境准备失败" : "正在准备创作环境……"}</h1>
        <p>
          {error ?? "正在检查本地数据、工作流目录和 ComfyUI 连接。ComfyUI 离线时仍可进入工作台。"}
        </p>
        {error && (
          <button type="button" className="primary-action" onClick={onRetry}>
            重试
          </button>
        )}
      </div>
    </main>
  );
}
