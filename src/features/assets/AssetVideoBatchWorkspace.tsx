import type { AssetView } from "../../types/asset";
import type { RecipeViewModel } from "../../types/generation";
import type { ProductionAdmissionStatus } from "../../types/productionQueue";
import type { H3GenerationMode } from "./assetVideoBatch";
import { isAudioAssetForH3, isImageAssetForVideo, isVideoAssetForH3 } from "./assetVideoBatch";
import { ProductionQueuePanel } from "../studio/ProductionQueuePanel";

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  initialAssets: AssetView[];
  comfyConnected: boolean;
  taskEventsReady: boolean;
  productionAdmission: ProductionAdmissionStatus;
  focusProductionBatchId?: string;
  onAdmissionChanged: () => Promise<void>;
  onProductionBatchFocused: () => void;
  onOpenTask: (taskId: string) => void;
  onOpenProductionQueue?: (batchId?: string) => void;
  onBackToAssets: () => void;
  onOpenWorkflows?: () => void;
}

interface ProjectFolderImportControlsProps {
  busy: boolean;
  hasInspection: boolean;
  onRescan: () => void;
}

/** Kept for historical import/control tests; local-folder auto-generate path is retired. */
export function ProjectFolderImportControls({ busy, hasInspection, onRescan }: ProjectFolderImportControlsProps) {
  return (
    <div className="h3-local-import-controls" aria-label="项目文件夹导入规则">
      <div className="h3-project-folder-policy">
        <strong>项目文件夹 · 分段自动识别</strong>
        <span>每个一级子文件夹对应一个视频分段；系统按提示词和媒体自动选择生成模式。</span>
      </div>
      {hasInspection && (
        <button type="button" className="quiet-button" onClick={onRescan} disabled={busy}>重新扫描</button>
      )}
    </div>
  );
}

export function projectParameterSourceLabel(source: string): string {
  switch (source) {
    case "USER_OVERRIDE": return "用户修改";
    case "FRONT_MATTER": return "前置元数据";
    case "PROMPT_SPEC": return "提示词";
    case "PROMPT_SPEC_ROUNDED": return "提示词取整";
    case "REFERENCE_VIDEO": return "参考视频";
    case "SOURCE_ASPECT": return "素材比例";
    case "RECIPE_DEFAULT": return "默认值";
    default: return source;
  }
}

export function h3InitialGenerationMode(initialAssets: AssetView[]): H3GenerationMode {
  return initialAssets.length === 1 && isImageAssetForVideo(initialAssets[0])
    ? "FL2VA_IMAGE_TO_VIDEO"
    : "FL2VA_TEXT_TO_VIDEO";
}

export function h3PickerAssets(assets: AssetView[], mode: H3GenerationMode): AssetView[] {
  switch (mode) {
    case "FL2VA_IMAGE_TO_VIDEO":
    case "FL2VA_FIRST_LAST":
    case "REF2VA_IMAGE":
      return assets.filter(isImageAssetForVideo);
    case "REF2VA_AUDIO":
      return assets.filter(isAudioAssetForH3);
    case "REF2VA_IMAGE_AUDIO":
      return assets.filter((asset) => isImageAssetForVideo(asset) || isAudioAssetForH3(asset));
    case "REF2VA_VIDEO_IMAGE":
      return assets.filter((asset) => isImageAssetForVideo(asset) || isVideoAssetForH3(asset));
    case "FL2VA_TEXT_TO_VIDEO":
      return [];
  }
}

/** Deprecated: local-only inputs are not authority; no auto-start. Use ShotBatchAssetBindPanel. */
export function AssetVideoBatchWorkspace({
  projectId, catalog: _c, initialAssets: _i, comfyConnected, taskEventsReady: _t,
  productionAdmission, focusProductionBatchId, onAdmissionChanged, onProductionBatchFocused,
  onOpenTask, onOpenProductionQueue, onBackToAssets, onOpenWorkflows: _w,
}: Props) {
  void _c; void _i; void _t; void _w;
  return (
    <section className="workspace-panel asset-video-batch-workspace" aria-label="已停用的本地批量视频工作区">
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">批量视频</span>
          <h2>请改用镜头正式输入槽</h2>
          <p className="section-description">
            本工作区不再使用本地临时选择作为输入权威，也不会创建或自动启动生产队列。
            请在镜头工作区打开「批量绑定素材」，把外部导入素材写入 ShotVideoInput（OCC），
            再通过视频批量准备生成待启动批次；由 Production Queue 作为唯一执行权威手动开始。
          </p>
        </div>
        <button type="button" className="quiet-button" onClick={onBackToAssets}>返回资产库</button>
      </div>
      <div className="asset-video-batch-deprecated" role="status">
        <strong>已停用：本地输入权威 / 自动开跑</strong>
        <p>正式路径：镜头结构 → 批量绑定素材（ShotBatchAssetBindPanel）→ 视频准备 → 生产队列手动开始。</p>
        {onOpenProductionQueue && <button type="button" onClick={() => onOpenProductionQueue()}>打开生产队列</button>}
      </div>
      <div className="model-version-selector" hidden aria-hidden="true" />
      <ProductionQueuePanel
        projectId={projectId}
        batchItems={[]}
        comfyConnected={comfyConnected}
        variant="inline"
        focusBatchId={focusProductionBatchId
          ?? (productionAdmission.projectId === projectId ? productionAdmission.batchId : undefined)}
        onAdmissionChanged={onAdmissionChanged}
        onFocusedBatchOpened={() => { if (focusProductionBatchId) onProductionBatchFocused(); }}
        onOpenTask={onOpenTask}
        onOpenProductionQueue={onOpenProductionQueue}
        hideCreate
      />
    </section>
  );
}
