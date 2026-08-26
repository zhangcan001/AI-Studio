import { useState } from "react";
import type { AssetView } from "../../types/asset";
import { AssetLibrary } from "./AssetLibrary";
import { ConsistencyProfileLibrary } from "./ConsistencyProfileLibrary";
import { ReferenceSetLibrary } from "./ReferenceSetLibrary";

export type AssetWorkspaceTab = "assets" | "profiles" | "referenceSets";

interface Props {
  projectId: string;
  onUseInStudio: (asset: AssetView) => void;
  onOpenVideoBatch: (assets: AssetView[]) => void;
  onOpenTask: (taskId: string) => void;
}

const tabs: Array<{ value: AssetWorkspaceTab; label: string; description: string }> = [
  { value: "assets", label: "素材", description: "源素材、生成结果与旧版参考锚点" },
  { value: "profiles", label: "档案", description: "角色、场景、道具与风格 Profile" },
  { value: "referenceSets", label: "参考集", description: "有序图片集合与使用关系" },
];

export function AssetWorkspace({ projectId, onUseInStudio, onOpenVideoBatch, onOpenTask }: Props) {
  const [activeTab, setActiveTab] = useState<AssetWorkspaceTab>("assets");

  return (
    <div className="asset-workspace" aria-label="资产工作区" style={{ display: "grid", gap: 12, minWidth: 0 }}>
      <nav className="asset-workspace-tabs" aria-label="资产工作区标签" role="tablist" style={{ display: "flex", alignItems: "stretch", gap: 6, flexWrap: "wrap", padding: 7, border: "1px solid var(--studio-border, rgba(255,255,255,.08))", borderRadius: 8, background: "var(--studio-surface-2, #14171c)" }}>
        {tabs.map((tab) => (
          <button
            key={tab.value}
            type="button"
            role="tab"
            aria-selected={activeTab === tab.value}
            className={activeTab === tab.value ? "workspace-nav-button workspace-nav-button-active" : "workspace-nav-button"}
            onClick={() => setActiveTab(tab.value)}
            style={{ display: "grid", gap: 2, flex: "1 1 180px", minWidth: 0, justifyItems: "start", textAlign: "left" }}
          >
            <strong>{tab.label}</strong>
            <small style={{ color: "var(--studio-text-secondary, #9ca3af)", fontWeight: 400, overflow: "hidden", textOverflow: "ellipsis", maxWidth: "100%" }}>{tab.description}</small>
          </button>
        ))}
      </nav>

      {activeTab === "assets" && <AssetLibrary projectId={projectId} onUseInStudio={onUseInStudio} onOpenVideoBatch={onOpenVideoBatch} onOpenTask={onOpenTask} />}
      {activeTab === "profiles" && <ConsistencyProfileLibrary projectId={projectId} />}
      {activeTab === "referenceSets" && <ReferenceSetLibrary projectId={projectId} />}
    </div>
  );
}

export { tabs };
