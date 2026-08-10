export type StudioMode = "single" | "batch" | "experiment";

interface Props {
  mode: StudioMode;
  onChange: (mode: StudioMode) => void;
}

export function StudioModeTabs({ mode, onChange }: Props) {
  return (
    <div className="studio-mode-tabs" role="tablist" aria-label="创作模式">
      <button type="button" role="tab" aria-selected={mode === "single"} className={mode === "single" ? "studio-mode-tab studio-mode-tab-active" : "studio-mode-tab"} onClick={() => onChange("single")}>
        单次创作
        <small>填写一次，立即生成</small>
      </button>
      <button type="button" role="tab" aria-selected={mode === "batch"} className={mode === "batch" ? "studio-mode-tab studio-mode-tab-active" : "studio-mode-tab"} onClick={() => onChange("batch")}>
        批量生产
        <small>管理任务清单和生产队列</small>
      </button>
      <button type="button" role="tab" aria-selected={mode === "experiment"} className={mode === "experiment" ? "studio-mode-tab studio-mode-tab-active" : "studio-mode-tab"} onClick={() => onChange("experiment")}>
        实验
        <small>参数变体与结果择优</small>
      </button>
    </div>
  );
}
