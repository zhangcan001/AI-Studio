interface Props {
  creating: boolean;
  canGenerate: boolean;
  canAddToBatch: boolean;
  blockedReason?: string;
  batchCount: number;
  onGenerate: () => void;
  onAddToBatch: () => void;
}

export function GenerationActionBar({
  creating,
  canGenerate,
  canAddToBatch,
  blockedReason,
  batchCount,
  onGenerate,
  onAddToBatch,
}: Props) {
  return (
    <section className="generation-action-bar" aria-label="生成操作">
      <div className="generation-action-copy">
        <span className="section-label">准备好了吗</span>
        <strong>保持输入，随时可以再次生成</strong>
        {blockedReason && <p role="status">{blockedReason}</p>}
      </div>
      <div className="generation-action-buttons">
        <button type="button" className="generation-primary-button" onClick={onGenerate} disabled={!canGenerate || creating}>
          {creating ? "正在创建任务..." : "开始生成"}
        </button>
        <button type="button" className="quiet-button" onClick={onAddToBatch} disabled={!canAddToBatch || creating}>
          加入批量清单{batchCount ? `（${batchCount}）` : ""}
        </button>
      </div>
    </section>
  );
}
