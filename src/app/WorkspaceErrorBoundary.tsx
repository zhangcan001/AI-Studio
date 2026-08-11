import { Component, Fragment, type ErrorInfo, type ReactNode } from "react";

interface Props {
  children: ReactNode;
  onBackToAssets: () => void;
  onRetry: () => void;
  resetKey?: string;
}

interface State {
  hasError: boolean;
  retryNonce: number;
}

interface FallbackProps {
  onBackToAssets: () => void;
  onRetry: () => void;
}

export function WorkspaceErrorFallback({ onBackToAssets, onRetry }: FallbackProps) {
  return (
    <section className="workspace-panel workspace-error-boundary" role="alert" aria-labelledby="workspace-error-title">
      <span className="section-label">工作区错误</span>
      <h2 id="workspace-error-title">批量视频页面发生异常。</h2>
      <p>当前工作区没有加载完成，其他工作区仍然可以继续使用。</p>
      <div className="workspace-error-actions">
        <button type="button" onClick={onBackToAssets}>返回资产库</button>
        <button type="button" className="quiet-button" onClick={onRetry}>重新打开批量视频</button>
      </div>
    </section>
  );
}

export class WorkspaceErrorBoundary extends Component<Props, State> {
  state: State = { hasError: false, retryNonce: 0 };

  static getDerivedStateFromError(): State {
    return { hasError: true, retryNonce: 0 };
  }

  componentDidCatch(_error: Error, _errorInfo: ErrorInfo) {
    // The fallback keeps the rest of the app available; the technical error remains in the console.
  }

  componentDidUpdate(previousProps: Props) {
    if (this.state.hasError && previousProps.resetKey !== this.props.resetKey) {
      this.setState((current) => ({ hasError: false, retryNonce: current.retryNonce + 1 }));
    }
  }

  private retry = () => {
    this.setState((current) => ({ hasError: false, retryNonce: current.retryNonce + 1 }));
    this.props.onRetry();
  };

  render() {
    if (this.state.hasError) {
      return <WorkspaceErrorFallback onBackToAssets={this.props.onBackToAssets} onRetry={this.retry} />;
    }
    return <Fragment key={this.state.retryNonce}>{this.props.children}</Fragment>;
  }
}
