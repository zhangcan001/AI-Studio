import { Component, Fragment, useEffect, useRef, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
  onBackToAssets: () => void;
  onRetry: () => void;
  resetKey?: string;
  workspaceName?: string;
}

interface State {
  hasError: boolean;
  retryNonce: number;
  errorMessage?: string;
}

interface FallbackProps {
  onBackToAssets: () => void;
  onRetry: () => void;
  workspaceName?: string;
  errorMessage?: string;
}

export function WorkspaceErrorFallback({
  onBackToAssets,
  onRetry,
  workspaceName = "当前工作区",
  errorMessage,
}: FallbackProps) {
  return (
    <section className="workspace-panel workspace-error-boundary" role="alert" aria-labelledby="workspace-error-title">
      <span className="section-label">工作区异常</span>
      <h2 id="workspace-error-title">{workspaceName}发生异常。</h2>
      <p>当前页面未能正常渲染，已自动隔离故障，其他工作区与本地数据不受影响。</p>
      {errorMessage && (
        <details className="settings-quiet-help">
          <summary>查看技术详情</summary>
          <pre style={{ whiteSpace: "pre-wrap", wordBreak: "break-word", fontSize: "12px" }}>{errorMessage}</pre>
        </details>
      )}
      <div className="workspace-error-actions">
        <button type="button" onClick={onRetry}>重新加载{workspaceName}</button>
        <button type="button" className="quiet-button" onClick={onBackToAssets}>返回素材库</button>
        {errorMessage && (
          <button
            type="button"
            className="quiet-button"
            onClick={() => void navigator.clipboard?.writeText(errorMessage)}
          >
            复制错误信息
          </button>
        )}
      </div>
    </section>
  );
}

export class WorkspaceErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, retryNonce: 0, errorMessage: undefined };

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, retryNonce: 0, errorMessage: error?.message || String(error) };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("[WorkspaceErrorBoundary]", this.props.workspaceName ?? "workspace", error, errorInfo);
  }

  componentDidUpdate(previousProps: Props) {
    if (this.state.hasError && previousProps.resetKey !== this.props.resetKey) {
      this.setState((current) => ({ hasError: false, retryNonce: current.retryNonce + 1, errorMessage: undefined }));
    }
  }

  private retry = () => {
    this.setState((current) => ({ hasError: false, retryNonce: current.retryNonce + 1, errorMessage: undefined }));
    this.props.onRetry();
  };

  render() {
    if (this.state.hasError) {
      return (
        <WorkspaceErrorFallback
          onBackToAssets={this.props.onBackToAssets}
          onRetry={this.retry}
          workspaceName={this.props.workspaceName}
          errorMessage={this.state.errorMessage}
        />
      );
    }
    return <Fragment key={this.state.retryNonce}>{this.props.children}</Fragment>;
  }
}

export function ThemedConfirmDialog({
  open,
  title,
  message,
  confirmLabel = "确认",
  cancelLabel = "取消",
  danger = false,
  onConfirm,
  onCancel,
}: {
  open: boolean;
  title: string;
  message: string;
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) {
      if (typeof dialog.showModal === "function") dialog.showModal();
      else dialog.setAttribute("open", "");
    }
    if (!open && dialog.open) {
      if (typeof dialog.close === "function") dialog.close();
      else dialog.removeAttribute("open");
    }
  }, [open]);
  if (!open) return null;
  return (
    <dialog
      ref={ref}
      className="themed-confirm-dialog"
      aria-label={title}
      onCancel={(e) => {
        e.preventDefault();
        onCancel();
      }}
    >
      <div className="themed-confirm-body">
        <h3>{title}</h3>
        <p>{message}</p>
        <div className="themed-confirm-actions">
          <button type="button" className="quiet-button" onClick={onCancel}>{cancelLabel}</button>
          <button type="button" className={danger ? "danger-button" : "primary-action"} onClick={onConfirm}>{confirmLabel}</button>
        </div>
      </div>
    </dialog>
  );
}
