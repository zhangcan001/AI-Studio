import type { ProductionAdmissionStatus } from "../../types/productionQueue";
import { ProductionQueuePanel } from "../studio/ProductionQueuePanel";

interface Props {
  projectId: string;
  comfyConnected: boolean;
  productionAdmission: ProductionAdmissionStatus;
  focusProductionBatchId?: string;
  onAdmissionChanged: () => Promise<void>;
  onOpenTask: (taskId: string) => void;
  onOpenProductionQueue?: (batchId?: string) => void;
  onBackToAssets: () => void;
}

/** Deprecated: local-only inputs are not authority; no auto-start. Use ShotBatchAssetBindPanel. */
export function AssetVideoBatchWorkspace({
  projectId, comfyConnected,
  productionAdmission, focusProductionBatchId, onAdmissionChanged,
  onOpenTask, onOpenProductionQueue, onBackToAssets,
}: Props) {
  return (
    <section className="workspace-panel asset-video-batch-workspace" aria-label="已停用的本地批量视频工作区">
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">批量视频</span>
          <h2>请改用镜头正式输入槽</h2>
          <p className="section-description">请在分镜工作台绑定素材并准备视频批次，再从生产队列手动开始。</p>
        </div>
        <button type="button" className="quiet-button" onClick={onBackToAssets}>返回资产库</button>
      </div>
      <div className="asset-video-batch-deprecated" role="status">
        <strong>旧版批量入口已停用</strong>
        {onOpenProductionQueue && <button type="button" onClick={() => onOpenProductionQueue()}>打开生产队列</button>}
      </div>
      <ProductionQueuePanel
        projectId={projectId}
        batchItems={[]}
        comfyConnected={comfyConnected}
        variant="inline"
        focusBatchId={focusProductionBatchId
          ?? (productionAdmission.projectId === projectId ? productionAdmission.batchId : undefined)}
        onFocusedBatchOpened={() => undefined}
        onAdmissionChanged={onAdmissionChanged}
        onOpenTask={onOpenTask}
        onOpenProductionQueue={onOpenProductionQueue}
        hideCreate
      />
    </section>
  );
}
