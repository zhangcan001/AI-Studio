interface Props {
  refreshing: boolean;
  notice?: string | null;
  onOpenWorkflows: () => void;
  onReconnectComfy: () => void;
  onRefresh: () => void;
}

export function NoWorkflowGuide({
  refreshing,
  notice,
  onOpenWorkflows,
  onReconnectComfy,
  onRefresh,
}: Props) {
  return (
    <section className="studio-empty no-workflow-guide" aria-labelledby="no-workflow-title">
      <span className="section-label">创作准备</span>
      <h2 id="no-workflow-title">还没有可用于创作的工作流</h2>
      <p>请按以下步骤准备本地创作环境：</p>
      <ol>
        <li>启动 ComfyUI，并确认接口可以连接。</li>
        <li>导入 API 工作流，或从已有备份恢复。</li>
        <li>返回创作页面，选择工作流并开始生成。</li>
      </ol>
      <div className="no-workflow-actions">
        <button type="button" className="primary-action" onClick={onOpenWorkflows}>
          前往工作流管理
        </button>
        <button type="button" onClick={onReconnectComfy}>
          测试 ComfyUI 连接
        </button>
        <button type="button" className="quiet-button" onClick={onRefresh} disabled={refreshing}>
          {refreshing ? "正在刷新……" : "刷新工作流"}
        </button>
      </div>
      {notice && <p className="error-message">{notice}</p>}
    </section>
  );
}
