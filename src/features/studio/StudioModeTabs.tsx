export type StudioMode = "single" | "batch" | "production" | "experiment";

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
      <button type="button" role="tab" aria-selected={mode === "production"} className={mode === "production" ? "studio-mode-tab studio-mode-tab-active" : "studio-mode-tab"} onClick={() => onChange("production")}>
        生产运行
        <small>提示词 → Krea2 → 选图 → H3</small>
      </button>
      <button type="button" role="tab" aria-selected={mode === "experiment"} className={mode === "experiment" ? "studio-mode-tab studio-mode-tab-active" : "studio-mode-tab"} onClick={() => onChange("experiment")}>
        基准实验室
        <small>工作流 / 配方 / 预设横向比较</small>
      </button>
    </div>
  );
}
