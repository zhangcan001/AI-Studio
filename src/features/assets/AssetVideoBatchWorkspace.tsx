import { useEffect, useMemo, useState } from "react";
import { createProductionQueue, listAssetVideoPrompts, setAssetVideoPrompt, startProductionQueue } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { RecipeViewModel } from "../../types/generation";
import type { ProductionAdmissionStatus } from "../../types/productionQueue";
import { toUserMessage } from "../../i18n/errorMessages";
import { MINIMAX_H3_WORKFLOW_ID } from "../runtime/productRuntimeScope";
import { buildH3BatchValues, h3PromptField, h3ReferenceField, isImageAssetForVideo } from "./assetVideoBatch";
import { ProductionQueuePanel } from "../studio/ProductionQueuePanel";
import type { BatchDraftItem } from "../studio/batchDraft";

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  initialAssets: AssetView[];
  comfyConnected: boolean;
  taskEventsReady: boolean;
  productionAdmission: ProductionAdmissionStatus;
  onAdmissionChanged: () => Promise<void>;
  onOpenTask: (taskId: string) => void;
  onBackToAssets: () => void;
}

export function AssetVideoBatchWorkspace({
  projectId,
  catalog,
  initialAssets,
  comfyConnected,
  taskEventsReady,
  productionAdmission,
  onAdmissionChanged,
  onOpenTask,
  onBackToAssets,
}: Props) {
  const recipe = useMemo(
    () => catalog.find((item) => item.workflowId === MINIMAX_H3_WORKFLOW_ID && item.outputTypes?.includes("video")),
    [catalog],
  );
  const promptField = recipe ? h3PromptField(recipe) : undefined;
  const referenceField = recipe ? h3ReferenceField(recipe) : undefined;
  const [prompts, setPrompts] = useState<Record<string, string>>({});
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set(initialAssets.map((asset) => asset.id)));
  const [savedIds, setSavedIds] = useState<Set<string>>(new Set());
  const [createdBatchId, setCreatedBatchId] = useState<string>();
  const [createdBatchStarted, setCreatedBatchStarted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string>();

  useEffect(() => {
    let active = true;
    setPrompts({});
    setSavedIds(new Set());
    setSelectedIds(new Set(initialAssets.map((asset) => asset.id)));
    if (!initialAssets.length) return () => { active = false; };
    void listAssetVideoPrompts(projectId, initialAssets.map((asset) => asset.id))
      .then((records) => {
        if (!active) return;
        setPrompts(Object.fromEntries(records.map((record) => [record.assetId, record.promptText])));
        setSavedIds(new Set(records.map((record) => record.assetId)));
      })
      .catch((error: unknown) => {
        if (active) setNotice(toUserMessage(error));
      });
    return () => { active = false; };
  }, [initialAssets, projectId]);

  const selectedAssets = initialAssets.filter((asset) => selectedIds.has(asset.id));
  const imageAssets = selectedAssets.filter(isImageAssetForVideo);
  const missingPromptAssets = imageAssets.filter((asset) => !prompts[asset.id]?.trim());
  const runtimeReady = Boolean(recipe && referenceField && promptField && comfyConnected && taskEventsReady);
  const canCreate = Boolean(
    runtimeReady &&
      !productionAdmission.busy &&
      imageAssets.length > 0 &&
      missingPromptAssets.length === 0 &&
      imageAssets.length <= 100,
  );
  const batchItems: BatchDraftItem[] = recipe
    ? imageAssets.map((asset) => ({
      id: asset.id,
      workflowName: recipe.name,
      workflowVersionId: recipe.workflowVersionId,
      recipeId: recipe.recipeId,
      values: buildH3BatchValues(recipe, asset.id, prompts[asset.id] ?? ""),
    }))
    : [];

  function toggleAsset(assetId: string) {
    setSelectedIds((current) => {
      const next = new Set(current);
      if (next.has(assetId)) next.delete(assetId);
      else if (next.size < 100) next.add(assetId);
      return next;
    });
  }

  function updatePrompt(assetId: string, value: string) {
    setPrompts((current) => ({ ...current, [assetId]: value }));
    setSavedIds((current) => {
      if (!current.has(assetId)) return current;
      const next = new Set(current);
      next.delete(assetId);
      return next;
    });
  }

  async function savePrompt(asset: AssetView) {
    setBusy(true); setNotice(undefined);
    try {
      await setAssetVideoPrompt(projectId, asset.id, prompts[asset.id] ?? "");
      setSavedIds((current) => new Set(current).add(asset.id));
      setNotice(`已保存「${asset.name}」的视频提示词。`);
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function createBatch() {
    if (!recipe || !canCreate) return;
    setBusy(true); setNotice(undefined);
    try {
      await Promise.all(imageAssets.map((asset) => setAssetVideoPrompt(projectId, asset.id, prompts[asset.id] ?? "")));
      setSavedIds(new Set(imageAssets.map((asset) => asset.id)));
      const created = await createProductionQueue({
        projectId,
        name: `批量视频 · ${new Date().toLocaleString()}`,
        continueOnFailure: true,
        items: batchItems.map((item) => ({
          workflowVersionId: item.workflowVersionId,
          recipeId: item.recipeId,
          values: item.values,
        })),
      });
      setCreatedBatchId(created.id);
      setCreatedBatchStarted(false);
      await onAdmissionChanged();
      try {
        await startProductionQueue(projectId, created.id);
        setCreatedBatchStarted(true);
        setNotice(`已创建并开始视频批次，共 ${created.total} 项；参数和视频提示词已冻结。`);
      } catch (startError: unknown) {
        setNotice(`已创建视频批次，共 ${created.total} 项；开始执行失败：${toUserMessage(startError)}。可手动开始。`);
      }
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function startBatch() {
    if (!createdBatchId) return;
    setBusy(true); setNotice(undefined);
    try {
      await startProductionQueue(projectId, createdBatchId);
      setCreatedBatchStarted(true);
      setNotice("视频批次已开始，队列将严格串行执行。");
      await onAdmissionChanged();
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="workspace-panel asset-video-batch-workspace" aria-busy={busy}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">批量视频</span>
          <h2>资产 + 视频提示词</h2>
          <p className="section-description">从当前项目图片资产直接创建 MiniMax H3 视频批次；它与图片批次相互独立。</p>
        </div>
        <button type="button" className="quiet-button" onClick={onBackToAssets}>返回资产库</button>
      </div>

      <section className="h3-safety-card" aria-label="H3 安全配置">
        <div>
          <strong>MiniMax H3 安全配置</strong>
          <p>0.1 MP · 1 秒 · 4 步 · 一次只运行一个项目</p>
        </div>
        <small>{recipe ? `运行时已锁定：${MINIMAX_H3_WORKFLOW_ID}` : "运行时未就绪"}</small>
      </section>

      {!initialAssets.length ? (
        <div className="asset-video-empty-state">
          <strong>还没有选择图片资产</strong>
          <p>回到资产库，勾选 1–100 张图片后点击“批量生成视频”。</p>
          <button type="button" onClick={onBackToAssets}>去资产库选择</button>
        </div>
      ) : (
        <>
          <div className="asset-video-batch-summary">
            <span>已选择 <strong>{selectedAssets.length}</strong> 项</span>
            <span>符合条件 <strong>{imageAssets.length - missingPromptAssets.length}</strong> 项</span>
            <span>未填写提示词 <strong>{missingPromptAssets.length}</strong> 项</span>
          </div>
          <div className="asset-video-batch-list" aria-label="视频生产素材列表">
            {initialAssets.map((asset, index) => {
              const isImage = isImageAssetForVideo(asset);
              const promptReady = Boolean(prompts[asset.id]?.trim());
              const checked = selectedIds.has(asset.id);
              const qualification = !isImage
                ? "不是图片素材"
                : !promptReady
                  ? "未填写视频提示词"
                  : !recipe
                    ? "H3 配方不可用"
                    : !comfyConnected
                      ? "ComfyUI 未连接"
                      : !taskEventsReady
                        ? "任务事件通道未就绪"
                        : "符合条件";
              return (
                <article key={asset.id} className={`asset-video-batch-card${checked ? " asset-video-batch-card-selected" : ""}`}>
                  <label className="asset-video-select-control">
                    <input type="checkbox" checked={checked} onChange={() => toggleAsset(asset.id)} disabled={busy || !isImage} />
                    <span>#{index + 1}</span>
                  </label>
                  <div className="asset-video-batch-card-body">
                    <div className="asset-video-batch-card-heading">
                      <strong>{asset.name}</strong>
                      <span className={qualification === "符合条件" ? "asset-prompt-status asset-prompt-status-ready" : "asset-prompt-status"}>
                        {qualification}
                      </span>
                    </div>
                    <label className="asset-video-prompt-input">
                      <span>视频提示词</span>
                      <textarea
                        rows={3}
                        maxLength={64 * 1024}
                        value={prompts[asset.id] ?? ""}
                        onChange={(event) => updatePrompt(asset.id, event.target.value)}
                        disabled={busy || !isImage}
                        placeholder="描述运动、方向或变化……"
                      />
                    </label>
                    <div className="asset-video-batch-card-actions">
                      <small>{savedIds.has(asset.id) && !promptReady ? "" : savedIds.has(asset.id) ? "提示词已保存" : "修改后请保存"}</small>
                      <button type="button" className="quiet-button" onClick={() => void savePrompt(asset)} disabled={busy || !isImage || !promptReady || savedIds.has(asset.id)}>
                        保存提示词
                      </button>
                    </div>
                  </div>
                </article>
              );
            })}
          </div>
          <div className="asset-video-batch-actions">
            <button type="button" onClick={() => void createBatch()} disabled={busy || !canCreate}>
              {busy ? "正在处理..." : `创建视频批次（${imageAssets.length}）`}
            </button>
            {createdBatchId && !createdBatchStarted && <button type="button" className="quiet-button" onClick={() => void startBatch()} disabled={busy || productionAdmission.busy}>开始生成</button>}
          </div>
          {!runtimeReady && <p className="error-message">H3 暂不可用：请确认 ComfyUI、任务事件通道和精确 H3 Recipe 已就绪。</p>}
          {missingPromptAssets.length > 0 && <p className="disabled-note">请先为所有已选图片保存视频提示词；批次创建时会冻结当前内容。</p>}
        </>
      )}
      {notice && <p className="studio-notice" role="status">{notice}</p>}
      <ProductionQueuePanel
        projectId={projectId}
        batchItems={[]}
        comfyConnected={comfyConnected}
        focusBatchId={createdBatchId}
        onAdmissionChanged={onAdmissionChanged}
        onFocusedBatchOpened={() => undefined}
        onOpenTask={onOpenTask}
        hideCreate
      />
    </section>
  );
}
