import { useEffect, useMemo, useState } from "react";
import {
  commitH3LocalImport,
  createProductionQueue,
  listAssetVideoPrompts,
  pickH3LocalImportDirectory,
  rescanH3LocalImport,
  setAssetVideoPrompt,
  startProductionQueue,
} from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { RecipeViewModel } from "../../types/generation";
import type { H3LocalImportInspection, H3LocalImportMode } from "../../types/h3LocalImport";
import type { ProductionAdmissionStatus } from "../../types/productionQueue";
import { toUserMessage } from "../../i18n/errorMessages";
import { MINIMAX_H3_WORKFLOW_ID } from "../runtime/productRuntimeScope";
import { ResolutionControl } from "../runtime/ResolutionControl";
import { MINIMAX_H3_RESOLUTION_PRESETS, resolutionPresetsForRecipe } from "../runtime/resolutionPresets";
import { validateResolution } from "../runtime/resolution";
import {
  buildH3BatchValues,
  canCreateH3Batch,
  h3AssetQualification,
  h3RecipeContract,
  isImageAssetForVideo,
} from "./assetVideoBatch";
import { ProductionQueuePanel } from "../studio/ProductionQueuePanel";
import type { BatchDraftItem } from "../studio/batchDraft";
import { formatPromptBytes, localImportCanCommit, localImportModeLabel, localImportStatusLabel } from "./h3LocalImport";

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
  onBackToAssets: () => void;
}

export function AssetVideoBatchWorkspace({
  projectId,
  catalog,
  initialAssets,
  comfyConnected,
  taskEventsReady,
  productionAdmission,
  focusProductionBatchId,
  onAdmissionChanged,
  onProductionBatchFocused,
  onOpenTask,
  onBackToAssets,
}: Props) {
  const [sourceMode, setSourceMode] = useState<"ASSET_LIBRARY" | "LOCAL_FOLDER">("ASSET_LIBRARY");
  const [localMode, setLocalMode] = useState<H3LocalImportMode>("PAIRING");
  const [localInspection, setLocalInspection] = useState<H3LocalImportInspection>();
  const [localBatchName, setLocalBatchName] = useState("");
  const [localAutoStart, setLocalAutoStart] = useState(true);
  const [expandedLocalOrdinal, setExpandedLocalOrdinal] = useState<number>();
  const recipe = useMemo(
    () => catalog.find((item) => item.workflowId === MINIMAX_H3_WORKFLOW_ID && item.outputTypes?.includes("video")),
    [catalog],
  );
  const contract = useMemo(
    () => recipe
      ? h3RecipeContract(recipe)
      : { ok: false as const, reason: "运行目录中没有精确的 MiniMax H3 Recipe。" },
    [recipe],
  );
  const [prompts, setPrompts] = useState<Record<string, string>>({});
  const [selectedIds, setSelectedIds] = useState<Set<string>>(() => new Set(initialAssets.map((asset) => asset.id)));
  const [savedIds, setSavedIds] = useState<Set<string>>(new Set());
  const [durationSeconds, setDurationSeconds] = useState<number>();
  const [width, setWidth] = useState<number>();
  const [height, setHeight] = useState<number>();
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

  useEffect(() => {
    setDurationSeconds(contract.ok ? contract.contract.durationField.default : undefined);
    setWidth(contract.ok ? contract.contract.widthField.default : undefined);
    setHeight(contract.ok ? contract.contract.heightField.default : undefined);
  }, [contract]);

  const selectedAssets = initialAssets.filter((asset) => selectedIds.has(asset.id));
  const imageAssets = selectedAssets.filter(isImageAssetForVideo);
  const missingPromptAssets = imageAssets.filter((asset) => !prompts[asset.id]?.trim());
  const oversizedPromptAssets = imageAssets.filter((asset) =>
    new TextEncoder().encode(prompts[asset.id] ?? "").byteLength > 64 * 1024,
  );
  const selectedDuration = contract.ok ? durationSeconds ?? contract.contract.durationField.default : undefined;
  const selectedWidth = contract.ok ? width ?? contract.contract.widthField.default : undefined;
  const selectedHeight = contract.ok ? height ?? contract.contract.heightField.default : undefined;
  const durationReady = Boolean(
    contract.ok
      && selectedDuration !== undefined
      && contract.contract.durationOptions.includes(selectedDuration),
  );
  const resolutionValidation = recipe && contract.ok
    ? validateResolution(recipe, selectedWidth, selectedHeight)
    : undefined;
  const resolutionReady = Boolean(resolutionValidation?.ok);
  const runtimeReady = Boolean(contract.ok && comfyConnected && taskEventsReady && durationReady && resolutionReady);
  const resolutionPresets = contract.ok && recipe
    ? resolutionPresetsForRecipe(recipe, MINIMAX_H3_RESOLUTION_PRESETS)
    : [];
  const exceedsHistoricalProfile = Boolean(
    selectedDuration !== undefined
      && selectedWidth !== undefined
      && selectedHeight !== undefined
      && (selectedDuration > 5 || selectedWidth * selectedHeight > 100_000),
  );
  const canCreate = canCreateH3Batch({
    runtimeReady,
    admissionBusy: productionAdmission.busy,
    imageCount: imageAssets.length,
    missingPromptCount: missingPromptAssets.length,
    oversizedPromptCount: oversizedPromptAssets.length,
  });
  const localCanCreate = localImportCanCommit(
    localInspection,
    runtimeReady,
    productionAdmission.busy,
  );
  const batchItems: BatchDraftItem[] = recipe && contract.ok && selectedDuration !== undefined && selectedWidth !== undefined && selectedHeight !== undefined
    ? imageAssets.map((asset) => ({
      id: asset.id,
      workflowName: recipe.name,
      workflowVersionId: recipe.workflowVersionId,
      recipeId: recipe.recipeId,
      values: buildH3BatchValues(recipe, asset.id, prompts[asset.id] ?? "", selectedDuration, selectedWidth, selectedHeight),
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
    if (!recipe || !contract.ok || selectedDuration === undefined || !canCreate) return;
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

  async function chooseLocalDirectory() {
    setBusy(true); setNotice(undefined);
    try {
      const inspection = await pickH3LocalImportDirectory(projectId, localMode);
      if (!inspection) return;
      setLocalInspection(inspection);
      setExpandedLocalOrdinal(undefined);
      setNotice(`已读取「${inspection.displayRootName}」，可生成 ${inspection.readyCount} 项。`);
    } catch (error: unknown) {
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function changeLocalMode(nextMode: H3LocalImportMode) {
    setLocalMode(nextMode);
    if (!localInspection) return;
    setBusy(true); setNotice(undefined);
    try {
      const inspection = await rescanH3LocalImport(localInspection.sessionId, nextMode);
      setLocalInspection(inspection);
      setExpandedLocalOrdinal(undefined);
    } catch (error: unknown) {
      setLocalInspection(undefined);
      setExpandedLocalOrdinal(undefined);
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function rescanLocalDirectory() {
    if (!localInspection) return;
    setBusy(true); setNotice(undefined);
    try {
      const inspection = await rescanH3LocalImport(localInspection.sessionId, localMode);
      setLocalInspection(inspection);
      setExpandedLocalOrdinal(undefined);
      setNotice(`已重新扫描，当前可生成 ${inspection.readyCount} 项。`);
    } catch (error: unknown) {
      setLocalInspection(undefined);
      setExpandedLocalOrdinal(undefined);
      setNotice(toUserMessage(error));
    } finally {
      setBusy(false);
    }
  }

  async function commitLocalBatch() {
    if (!recipe || !contract.ok || !localInspection || !localCanCreate) return;
    const selectedDuration = durationSeconds ?? contract.contract.durationField.default;
    const selectedWidth = width ?? contract.contract.widthField.default;
    const selectedHeight = height ?? contract.contract.heightField.default;
    if (selectedDuration === undefined || selectedWidth === undefined || selectedHeight === undefined) return;
    setBusy(true); setNotice(undefined);
    try {
      const result = await commitH3LocalImport({
        sessionId: localInspection.sessionId,
        batchName: localBatchName.trim() || undefined,
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        width: selectedWidth,
        height: selectedHeight,
        durationSeconds: selectedDuration,
        autoStart: localAutoStart,
      });
      setCreatedBatchId(result.batchId);
      setCreatedBatchStarted(result.autoStarted);
      setLocalInspection(undefined);
      setExpandedLocalOrdinal(undefined);
      await onAdmissionChanged();
      setNotice(
        `本地任务已导入，共${result.itemCount}项。${result.autoStarted ? "已开始生成；" : "已创建批次；"}图片已进入资产库。${result.warnings.length ? ` ${result.warnings.join("；")}` : ""}`,
      );
    } catch (error: unknown) {
      setLocalInspection(undefined);
      setExpandedLocalOrdinal(undefined);
      setNotice(`导入未完成，请重新选择任务目录。${toUserMessage(error)}`);
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

      <div className="asset-video-source-tabs" role="tablist" aria-label="视频批量输入来源">
        <button
          type="button"
          role="tab"
          aria-selected={sourceMode === "ASSET_LIBRARY"}
          className={sourceMode === "ASSET_LIBRARY" ? "asset-video-source-tab asset-video-source-tab-active" : "asset-video-source-tab"}
          onClick={() => setSourceMode("ASSET_LIBRARY")}
          disabled={busy}
        >
          从资产库
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={sourceMode === "LOCAL_FOLDER"}
          className={sourceMode === "LOCAL_FOLDER" ? "asset-video-source-tab asset-video-source-tab-active" : "asset-video-source-tab"}
          onClick={() => setSourceMode("LOCAL_FOLDER")}
          disabled={busy}
        >
          从本地导入
        </button>
      </div>

      <section className="h3-safety-card" aria-label="H3 安全配置">
        <div>
          <strong>MiniMax H3</strong>
          <p>模型产品能力：最高 15 秒 · 最高 2K</p>
          <p>当前 Runtime：4 步 · 单任务串行</p>
          <small>本机历史已验证：0.1 MP · 5 秒 · RTX 5060 Ti 16GB</small>
        </div>
        {contract.ok && (
          <ResolutionControl
            widthField={contract.contract.widthField}
            heightField={contract.contract.heightField}
            width={selectedWidth}
            height={selectedHeight}
            presets={resolutionPresets}
            disabled={busy}
            onChange={(next) => {
              setWidth(next.width);
              setHeight(next.height);
            }}
          />
        )}
        <div className="h3-duration-control">
          <label htmlFor="h3-duration-seconds">视频时长</label>
          <select
            id="h3-duration-seconds"
            value={selectedDuration ?? ""}
            onChange={(event) => setDurationSeconds(Number(event.target.value))}
            disabled={busy || !contract.ok}
          >
            <option value="" disabled>Recipe 不可用</option>
            {contract.ok && contract.contract.durationOptions.map((option) => (
              <option key={option} value={option}>{option} 秒</option>
            ))}
          </select>
          <small>
            {contract.ok
              ? `Recipe 范围 ${contract.contract.durationField.min}–${contract.contract.durationField.max} 秒 · 默认 ${contract.contract.durationField.default} 秒`
              : "H3 runtime unavailable"}
          </small>
        </div>
        <small>{recipe ? `运行时已锁定：${MINIMAX_H3_WORKFLOW_ID}` : "运行时未就绪"}</small>
      </section>
      {exceedsHistoricalProfile && (
        <p className="h3-profile-warning" role="status">
          当前配置超出本机已验证范围。模型/产品允许该配置，但 RTX 5060 Ti 16GB 的显存占用尚未验证，生成时可能出现显存不足。
        </p>
      )}

      {sourceMode === "LOCAL_FOLDER" ? (
        <div className="h3-local-import-layout">
          <section className="h3-local-import-panel" aria-label="MiniMax H3 本地批量导入">
            <div className="section-heading">
              <div>
                <span className="section-label">本地批量</span>
                <h3>MiniMax H3 本地导入</h3>
                <p className="section-description">选择任务目录，检查图片与 Prompt 后导入资产库并创建普通视频生产队列。</p>
              </div>
              <button type="button" onClick={() => void chooseLocalDirectory()} disabled={busy}>
                {busy ? "处理中…" : "选择任务目录"}
              </button>
            </div>

            <div className="h3-local-import-controls">
              <label>
                <span>导入方式</span>
                <select value={localMode} onChange={(event) => void changeLocalMode(event.target.value as H3LocalImportMode)} disabled={busy}>
                  <option value="PAIRING">{localImportModeLabel("PAIRING")}</option>
                  <option value="MANIFEST">{localImportModeLabel("MANIFEST")}</option>
                </select>
              </label>
              {localInspection && (
                <button type="button" className="quiet-button" onClick={() => void rescanLocalDirectory()} disabled={busy}>
                  重新扫描
                </button>
              )}
            </div>

            {!localInspection ? (
              <div className="h3-local-import-empty">
                <strong>尚未选择任务目录</strong>
                <span>目录只在本机短时使用；页面不会显示或保存绝对路径。</span>
              </div>
            ) : (
              <>
                <div className="h3-local-import-root">
                  <span>当前目录</span>
                  <strong>{localInspection.displayRootName}</strong>
                </div>
                <div className="h3-local-import-summary" aria-label="本地批量扫描结果">
                  <span>图片 <strong>{localInspection.imageCount}</strong></span>
                  <span>Prompt <strong>{localInspection.promptCount}</strong></span>
                  <span className="h3-local-import-summary-ready">可生成 <strong>{localInspection.readyCount}</strong></span>
                  <span className={localInspection.errorCount ? "h3-local-import-summary-error" : ""}>异常 <strong>{localInspection.errorCount}</strong></span>
                </div>
                {localInspection.detectedManifest && localMode === "PAIRING" && (
                  <p className="h3-local-import-warning" role="status">检测到 h3-batch.json；当前使用自动同名配对，可切换为 JSON 批量清单。</p>
                )}
                {localInspection.warnings.map((warning) => <p key={warning} className="h3-local-import-warning" role="status">{warning}</p>)}
                {localInspection.errors.length > 0 && (
                  <div className="h3-local-import-errors" role="alert">
                    <strong>导入前需要修复：</strong>
                    <ul>{localInspection.errors.slice(0, 12).map((error) => <li key={error}>{error}</li>)}</ul>
                    {localInspection.errors.length > 12 && <small>还有 {localInspection.errors.length - 12} 项异常，请修复后重新扫描。</small>}
                  </div>
                )}
                <div className="h3-local-import-list" role="table" aria-label="本地批量项目列表">
                  <div className="h3-local-import-list-heading" role="row">
                    <span>序号</span><span>图片</span><span>提示词</span><span>字节</span><span>状态</span>
                  </div>
                  {localInspection.pairs.map((pair) => {
                    const expanded = expandedLocalOrdinal === pair.ordinal;
                    return (
                      <div className={`h3-local-import-row${pair.status === "READY" ? " h3-local-import-row-ready" : " h3-local-import-row-error"}`} role="row" key={`${pair.ordinal}-${pair.imageDisplayName}`}>
                        <span className="h3-local-import-ordinal">#{pair.ordinal}</span>
                        <span className="h3-local-import-file" title={pair.imageDisplayName}>{pair.imageDisplayName}</span>
                        <span className="h3-local-import-prompt">
                          <span className={expanded ? "" : "h3-local-import-prompt-preview"}>{pair.promptPreview ?? pair.promptDisplayName}</span>
                          {pair.promptPreview && <button type="button" className="quiet-button h3-local-import-prompt-toggle" onClick={() => setExpandedLocalOrdinal(expanded ? undefined : pair.ordinal)}>{expanded ? "收起" : "查看 Prompt"}</button>}
                        </span>
                        <span className="h3-local-import-bytes">{formatPromptBytes(pair.promptBytes)}</span>
                        <span className={pair.status === "READY" ? "h3-local-import-status h3-local-import-status-ready" : "h3-local-import-status"}>{localImportStatusLabel(pair.status)}</span>
                      </div>
                    );
                  })}
                </div>
                <div className="h3-local-import-options">
                  <label>
                    <span>批次名称（可选）</span>
                    <input value={localBatchName} onChange={(event) => setLocalBatchName(event.target.value)} maxLength={120} placeholder="H3 本地批量" disabled={busy} />
                  </label>
                  <label className="h3-local-import-checkbox">
                    <input type="checkbox" checked={localAutoStart} onChange={(event) => setLocalAutoStart(event.target.checked)} disabled={busy} />
                    <span>导入后立即开始生成</span>
                  </label>
                </div>
                <div className="asset-video-batch-actions">
                  <button type="button" onClick={() => void commitLocalBatch()} disabled={busy || !localCanCreate}>
                    {busy ? "正在导入…" : localAutoStart ? `导入并生成（${localInspection.readyCount}）` : `导入并创建批次（${localInspection.readyCount}）`}
                  </button>
                  {createdBatchId && !createdBatchStarted && <button type="button" className="quiet-button" onClick={() => void startBatch()} disabled={busy || productionAdmission.busy}>开始生成</button>}
                </div>
                {!runtimeReady && <p className="error-message" role="alert">H3 runtime unavailable：{contract.ok ? (!comfyConnected ? "ComfyUI 未连接。" : !taskEventsReady ? "任务事件通道未就绪。" : !durationReady ? "请选择有效的 Recipe 时长。" : !resolutionReady ? "请选择符合 Recipe 约束的输出分辨率。" : "运行时未就绪。") : contract.reason}</p>}
              </>
            )}
          </section>
          <ProductionQueuePanel
            projectId={projectId}
            batchItems={[]}
            comfyConnected={comfyConnected}
            variant="inline"
            focusBatchId={createdBatchId
              ?? focusProductionBatchId
              ?? (productionAdmission.projectId === projectId ? productionAdmission.batchId : undefined)}
            onAdmissionChanged={onAdmissionChanged}
            onFocusedBatchOpened={() => {
              if (focusProductionBatchId) onProductionBatchFocused();
            }}
            onOpenTask={onOpenTask}
            hideCreate
          />
        </div>
      ) : !initialAssets.length ? (
        <div className="asset-video-empty-state">
          <strong>还没有选择图片资产</strong>
          <p>回到资产库，勾选 1–100 张图片后点击“批量生成视频”。</p>
          <button type="button" onClick={onBackToAssets}>去资产库选择</button>
        </div>
      ) : (
        <div className="batch-workspace-grid">
          <div className="batch-editor-column">
          <div className="asset-video-batch-summary">
            <span>已选择 <strong>{selectedAssets.length}</strong> 项</span>
            <span>符合条件 <strong>{imageAssets.length - missingPromptAssets.length - oversizedPromptAssets.length}</strong> 项</span>
            <span>未填写提示词 <strong>{missingPromptAssets.length}</strong> 项</span>
          </div>
          <div className="asset-video-batch-list" aria-label="视频生产素材列表">
            {initialAssets.map((asset, index) => {
              const isImage = isImageAssetForVideo(asset);
              const promptReady = Boolean(prompts[asset.id]?.trim());
              const checked = selectedIds.has(asset.id);
              const qualification = h3AssetQualification({
                isImage,
                promptReady,
                promptTooLong: oversizedPromptAssets.some((item) => item.id === asset.id),
                h3RuntimeReady: contract.ok,
                comfyConnected,
                taskEventsReady,
                durationReady,
                resolutionReady,
                resolutionError: resolutionValidation?.errors.width ?? resolutionValidation?.errors.height,
              });
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
          {!runtimeReady && <p className="error-message" role="alert">H3 runtime unavailable：{contract.ok ? (!comfyConnected ? "ComfyUI 未连接。" : !taskEventsReady ? "任务事件通道未就绪。" : !durationReady ? "请选择有效的 Recipe 时长。" : !resolutionReady ? "请选择符合 Recipe 约束的输出分辨率。" : "运行时未就绪。") : contract.reason}</p>}
          {missingPromptAssets.length > 0 && <p className="disabled-note">请先为所有已选图片保存视频提示词；批次创建时会冻结当前内容。</p>}
          {oversizedPromptAssets.length > 0 && <p className="error-message">视频提示词按 UTF-8 计算不得超过 64 KiB，请缩短后再创建批次。</p>}
          </div>
          <ProductionQueuePanel
            projectId={projectId}
            batchItems={[]}
            comfyConnected={comfyConnected}
            variant="inline"
            focusBatchId={createdBatchId
              ?? focusProductionBatchId
              ?? (productionAdmission.projectId === projectId ? productionAdmission.batchId : undefined)}
            onAdmissionChanged={onAdmissionChanged}
            onFocusedBatchOpened={() => {
              if (focusProductionBatchId) onProductionBatchFocused();
            }}
            onOpenTask={onOpenTask}
            hideCreate
          />
        </div>
      )}
      {notice && <p className="studio-notice" role="status">{notice}</p>}
    </section>
  );
}
