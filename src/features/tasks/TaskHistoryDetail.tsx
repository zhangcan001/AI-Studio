import { useEffect, useState } from "react";
import { createGeneration, getReusableDraft } from "../../services/tauriClient";
import { useTaskStore } from "../../stores/taskStore";
import type { DraftValue } from "../../types/generation";
import type { ReusableGenerationDraft, RuntimeProvenance, TaskDetail, TaskNodeError } from "../../types/history";
import { AssetCard } from "../assets/AssetCard";
import { taskRetryDecision } from "./retryPolicy";
import { productionInteractionPolicy } from "../studio/productionQueuePolicy";
import { toUserMessage } from "../../i18n/errorMessages";
import { fieldLabel, formatDateTime, taskStatusLabel, workflowDisplayName } from "../../i18n/statusLabels";
import { UiErrorNotice } from "../../i18n/UiErrorNotice";

interface Props {
  projectId: string;
  detail: TaskDetail;
  loadingDraft: boolean;
  comfyConnected: boolean;
  productionBusy: boolean;
  onBack: () => void;
  onLoadInputs: (draft: ReusableGenerationDraft) => void;
  onOpenAsset: (assetId: string) => void;
}

export function TaskHistoryDetail({
  projectId,
  detail,
  loadingDraft: detailLoading,
  comfyConnected,
  productionBusy,
  onBack,
  onLoadInputs,
  onOpenAsset,
}: Props) {
  const [draft, setDraft] = useState<ReusableGenerationDraft>();
  const [draftLoading, setDraftLoading] = useState(detailLoading);
  const [draftError, setDraftError] = useState<string>();
  const [retrying, setRetrying] = useState(false);
  const [retryError, setRetryError] = useState<string>();
  const [retryCreatedTaskId, setRetryCreatedTaskId] = useState<string>();
  const retryDecision = taskRetryDecision(detail, comfyConnected);
  const productionPolicy = productionInteractionPolicy(productionBusy);

  useEffect(() => {
    if (!detail.reusableDraft.available) return;
    let active = true;
    setDraftLoading(true);
    setDraftError(undefined);
    void getReusableDraft(projectId, detail.id)
      .then((value) => {
        if (active) setDraft(value);
      })
      .catch((error: unknown) => {
        if (active) setDraftError(toUserMessage(error));
      })
      .finally(() => {
        if (active) setDraftLoading(false);
      });
    return () => {
      active = false;
    };
  }, [detail.id, detail.reusableDraft.available, projectId]);

  async function retryOnce() {
    if (!retryDecision.allowed || !draft || retryCreatedTaskId || !productionPolicy.canRetryTask) return;
    setRetrying(true);
    setRetryError(undefined);
    try {
      const task = await createGeneration({
        projectId,
        workflowVersionId: draft.workflowVersionId,
        recipeId: draft.recipeId,
        values: draft.values,
      });
      useTaskStore.getState().adoptCreatedTask(task);
      setRetryCreatedTaskId(task.id);
    } catch (error: unknown) {
      setRetryError(toUserMessage(error));
    } finally {
      setRetrying(false);
    }
  }

  return (
    <div className="task-detail-view">
      <button type="button" className="quiet-button back-button" onClick={onBack}>返回任务历史</button>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">任务详情</span>
          <h2>{workflowDisplayName(detail.workflowId, detail.workflowName)}</h2>
          <p className="section-description">{detail.id}</p>
        </div>
        <span className={`status-pill task-${detail.status.toLowerCase()}`}>{taskStatusLabel(detail.status)}</span>
      </div>
      <div className="detail-facts">
        <Fact label="创建时间" value={formatDateTime(detail.createdAt)} />
        <Fact label="开始时间" value={detail.startedAt ? formatDateTime(detail.startedAt) : "—"} />
        <Fact label="完成时间" value={detail.finishedAt ? formatDateTime(detail.finishedAt) : "—"} />
        <Fact label="配方 ID" value={detail.recipeId} />
      </div>
      {detail.errorCode && (
        <UiErrorNotice error={{ code: detail.errorCode, message: detail.errorMessage ?? "任务未成功完成。" }} className="task-error" />
      )}
      {detail.runtimeProvenance && <RuntimeDiagnostics provenance={detail.runtimeProvenance} />}
      {detail.errorCode === "WORKFLOW_VALIDATION_FAILED" && (
        <ComfyNodeErrorSection nodeErrors={detail.nodeErrors ?? []} rawError={detail.rawError} />
      )}
      {detail.status === "FAILED" && (
        <section className="task-retry-panel" aria-label="重试任务">
          <div>
            <span className="section-label">重试说明</span>
            <p>
              {retryCreatedTaskId
                ? `已创建重试任务：${retryCreatedTaskId}`
                : retryDecision.allowed
                  ? "该任务看起来是临时失败，可以使用已保存的输入创建一个新任务。"
                  : retryDecision.reason}
            </p>
          </div>
          <button
            type="button"
            onClick={() => void retryOnce()}
            disabled={
              !retryDecision.allowed ||
              !productionPolicy.canRetryTask ||
              retrying ||
              draftLoading ||
              !draft ||
              Boolean(retryCreatedTaskId)
            }
          >
            {retrying ? "正在创建重试任务..." : retryCreatedTaskId ? "重试任务已创建" : "重试一次"}
          </button>
          {productionBusy && (
            <p className="disabled-note">生产队列正在运行，重试一次暂时不可用。</p>
          )}
          {retryError && <p className="error-message">重试失败：{retryError}</p>}
        </section>
      )}
      <section className="detail-section">
        <div className="section-heading">
          <div>
            <span className="section-label">输入快照</span>
            <h3>已保存的输入</h3>
          </div>
          {detail.reusableDraft.available && (
            <button
              type="button"
              onClick={() => draft && onLoadInputs(draft)}
              disabled={draftLoading || !draft}
            >
              {draftLoading ? "正在加载到创作..." : "加载到创作"}
            </button>
          )}
        </div>
        {!detail.reusableDraft.available && (
          <p className="disabled-note">{detail.reusableDraft.reason ? toUserMessage(detail.reusableDraft.reason) : "保存的输入不可用于再次生成。"}</p>
        )}
        {detail.reusableDraft.missingAssetIds.length > 0 && (
          <p className="disabled-note">缺少媒体素材，加载输入后请选择替代素材。</p>
        )}
        {draftError && <p className="error-message">输入加载失败：{draftError}</p>}
        {draft && <InputSnapshot values={draft.values} />}
      </section>
      <section className="detail-section">
        <div className="section-heading">
          <div>
            <span className="section-label">输出结果</span>
            <h3>{detail.outputAssets.length} 个资产</h3>
          </div>
        </div>
        {detail.outputAssets.length ? (
          <div className="output-grid">
            {detail.outputAssets.map((asset) => (
              <AssetCard key={asset.id} projectId={projectId} asset={asset} onSelect={() => onOpenAsset(asset.id)} />
            ))}
          </div>
        ) : (
          <p className="empty-state">暂无输出资产。</p>
        )}
      </section>
    </div>
  );
}

function RuntimeDiagnostics({ provenance }: { provenance: RuntimeProvenance }) {
  return (
    <section className="detail-section runtime-diagnostics" aria-label="Runtime Diagnostics">
      <div className="section-heading">
        <div>
          <span className="section-label">Runtime Diagnostics</span>
          <h3>实际运行来源</h3>
        </div>
      </div>
      <div className="detail-facts">
        <Fact label="App version" value={provenance.appVersion} />
        <Fact label="Build commit" value={provenance.buildCommit} />
        <Fact label="Workflow package" value={provenance.packageName ?? "—"} />
        <Fact label="Workflow ID" value={provenance.workflowId} />
        <Fact label="Workflow version ID" value={provenance.workflowVersionId} />
        <Fact label="Workflow version" value={provenance.workflowVersion} />
        <Fact label="Workflow SHA-256" value={provenance.workflowSha256} />
        <Fact label="Recipe ID" value={provenance.recipeId} />
        <Fact label="Recipe version" value={provenance.recipeVersion} />
        <Fact label="Recipe SHA-256" value={provenance.recipeSha256} />
        <Fact label="Package source" value={provenance.packageSourcePath ?? "—"} />
        <Fact
          label="Dynamic binding targets"
          value={provenance.dynamicBindingTargets.length ? provenance.dynamicBindingTargets.join(" · ") : "—"}
        />
      </div>
    </section>
  );
}

function Fact({ label, value }: { label: string; value: string }) {
  return (
    <div className="detail-fact">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function InputSnapshot({ values }: { values: Record<string, DraftValue> }) {
  return (
    <dl className="input-snapshot">
      {Object.entries(values).map(([key, value]) => (
        <div key={key}>
          <dt>{fieldLabel(key)}</dt>
          <dd>{formatValue(value)}</dd>
        </div>
      ))}
    </dl>
  );
}

function ComfyNodeErrorSection({
  nodeErrors,
  rawError,
}: {
  nodeErrors: TaskNodeError[];
  rawError: unknown;
}) {
  const rawText = rawError === undefined ? "" : JSON.stringify(rawError, null, 2) ?? "";
  const [copied, setCopied] = useState(false);

  async function copyRawError() {
    if (!rawText || !navigator.clipboard) return;
    await navigator.clipboard.writeText(rawText);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  }

  return (
    <section className="detail-section comfy-node-error-section" aria-label="ComfyUI 节点校验详情">
      <div className="section-heading">
        <div>
          <span className="section-label">ComfyUI 校验详情</span>
          <h3>ComfyUI 节点校验详情</h3>
        </div>
      </div>
      {nodeErrors.length > 0 ? (
        <div className="comfy-node-error-list">
          {nodeErrors.map((error, index) => (
            <article className="comfy-node-error" key={`${error.nodeId}-${index}`}>
              <div className="comfy-node-error-heading">
                <strong>节点 {error.nodeId}</strong>
                {error.nodeType && <code>{error.nodeType}</code>}
              </div>
              <dl>
                {error.input && <ErrorFact label="输入" value={error.input} />}
                {error.errorType && <ErrorFact label="错误类型" value={error.errorType} />}
                <ErrorFact label="消息" value={error.message} />
                {error.details && <ErrorFact label="详情" value={error.details} />}
                {error.receivedValue !== undefined && (
                  <ErrorFact label="当前值" value={formatDiagnosticValue(error.receivedValue)} code />
                )}
                {error.expectedConfig !== undefined && (
                  <ErrorFact label="期望配置" value={formatDiagnosticValue(error.expectedConfig)} code />
                )}
              </dl>
            </article>
          ))}
        </div>
      ) : (
        <p className="disabled-note">ComfyUI 未返回结构化 node_errors；请查看上方原始错误消息。</p>
      )}
      {rawText && (
        <details className="comfy-raw-error">
          <summary>展开原始 JSON</summary>
          <div className="comfy-raw-error-actions">
            <button type="button" className="quiet-button" onClick={() => void copyRawError()}>
              {copied ? "已复制" : "复制"}
            </button>
          </div>
          <pre>{rawText}</pre>
        </details>
      )}
    </section>
  );
}

function ErrorFact({ label, value, code = false }: { label: string; value: string; code?: boolean }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd className={code ? "diagnostic-code" : undefined}>{value}</dd>
    </div>
  );
}

function formatDiagnosticValue(value: unknown): string {
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value);
  } catch {
    return String(value);
  }
}

function formatValue(value: DraftValue): string {
  switch (value.type) {
    case "string":
      return value.value || "（空）";
    case "integer":
      return String(value.value);
    case "number":
      return String(value.value);
    case "seed_random":
      return "随机种子";
    case "seed_fixed":
      return value.value;
    case "image_asset":
      return value.assetId;
    case "image_assets":
      return value.assetIds.length ? value.assetIds.join(" → ") : "暂无图片";
    case "video_asset":
      return value.assetId;
    case "audio_asset":
      return value.assetId;
    case "video_assets":
      return value.assetIds.length ? value.assetIds.join(" → ") : "暂无视频";
    case "audio_assets":
      return value.assetIds.length ? value.assetIds.join(" → ") : "暂无音频";
  }
}
