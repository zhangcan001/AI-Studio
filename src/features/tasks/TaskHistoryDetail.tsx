import { useEffect, useState } from "react";
import { createGeneration, getReusableDraft } from "../../services/tauriClient";
import { useTaskStore } from "../../stores/taskStore";
import type { DraftValue } from "../../types/generation";
import type { ReusableGenerationDraft, TaskDetail } from "../../types/history";
import { AssetCard } from "../assets/AssetCard";
import { taskRetryDecision } from "./retryPolicy";

interface Props {
  projectId: string;
  detail: TaskDetail;
  loadingDraft: boolean;
  comfyConnected: boolean;
  onBack: () => void;
  onLoadInputs: (draft: ReusableGenerationDraft) => void;
  onOpenAsset: (assetId: string) => void;
}

export function TaskHistoryDetail({
  projectId,
  detail,
  loadingDraft: detailLoading,
  comfyConnected,
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
        if (active) setDraftError(error instanceof Error ? error.message : String(error));
      })
      .finally(() => {
        if (active) setDraftLoading(false);
      });
    return () => {
      active = false;
    };
  }, [detail.id, detail.reusableDraft.available, projectId]);

  async function retryOnce() {
    if (!retryDecision.allowed || !draft || retryCreatedTaskId) return;
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
      setRetryError(error instanceof Error ? error.message : String(error));
    } finally {
      setRetrying(false);
    }
  }

  return (
    <div className="task-detail-view">
      <button type="button" className="quiet-button back-button" onClick={onBack}>Back to history</button>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">Task detail</span>
          <h2>{detail.workflowName}</h2>
          <p className="section-description">{detail.id}</p>
        </div>
        <span className={`status-pill task-${detail.status.toLowerCase()}`}>{detail.status}</span>
      </div>
      <div className="detail-facts">
        <Fact label="Created" value={formatDate(detail.createdAt)} />
        <Fact label="Started" value={detail.startedAt ? formatDate(detail.startedAt) : "—"} />
        <Fact label="Finished" value={detail.finishedAt ? formatDate(detail.finishedAt) : "—"} />
        <Fact label="Recipe" value={detail.recipeId} />
      </div>
      {detail.errorCode && (
        <div className="task-error">
          <strong>{detail.errorCode}</strong>
          <span>{detail.errorMessage ?? "The task did not complete successfully."}</span>
        </div>
      )}
      {detail.status === "FAILED" && (
        <section className="task-retry-panel" aria-label="Retry task">
          <div>
            <span className="section-label">Retry policy</span>
            <p>
              {retryCreatedTaskId
                ? `Retry task created: ${retryCreatedTaskId}`
                : retryDecision.allowed
                  ? "This looks transient. You can create one new task from the saved inputs."
                  : retryDecision.reason}
            </p>
          </div>
          <button
            type="button"
            onClick={() => void retryOnce()}
            disabled={
              !retryDecision.allowed ||
              retrying ||
              draftLoading ||
              !draft ||
              Boolean(retryCreatedTaskId)
            }
          >
            {retrying ? "Creating Retry..." : retryCreatedTaskId ? "Retry Created" : "Retry Once"}
          </button>
          {retryError && <p className="error-message">Retry failed: {retryError}</p>}
        </section>
      )}
      <section className="detail-section">
        <div className="section-heading">
          <div>
            <span className="section-label">Snapshot</span>
            <h3>Saved inputs</h3>
          </div>
          {detail.reusableDraft.available && (
            <button
              type="button"
              onClick={() => draft && onLoadInputs(draft)}
              disabled={draftLoading || !draft}
            >
              {draftLoading ? "Loading inputs..." : "Load Inputs"}
            </button>
          )}
        </div>
        {!detail.reusableDraft.available && (
          <p className="disabled-note">{detail.reusableDraft.reason ?? "Inputs unavailable for reuse."}</p>
        )}
        {detail.reusableDraft.missingAssetIds.length > 0 && (
          <p className="disabled-note">Missing media asset. Choose a replacement after loading inputs.</p>
        )}
        {draftError && <p className="error-message">Inputs unavailable for reuse.</p>}
        {draft && <InputSnapshot values={draft.values} />}
      </section>
      <section className="detail-section">
        <div className="section-heading">
          <div>
            <span className="section-label">Outputs</span>
            <h3>{detail.outputAssets.length} asset{detail.outputAssets.length === 1 ? "" : "s"}</h3>
          </div>
        </div>
        {detail.outputAssets.length ? (
          <div className="output-grid">
            {detail.outputAssets.map((asset) => (
              <AssetCard key={asset.id} projectId={projectId} asset={asset} onSelect={() => onOpenAsset(asset.id)} />
            ))}
          </div>
        ) : (
          <p className="empty-state">No output assets recorded.</p>
        )}
      </section>
    </div>
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
          <dt>{key}</dt>
          <dd>{formatValue(value)}</dd>
        </div>
      ))}
    </dl>
  );
}

function formatValue(value: DraftValue): string {
  switch (value.type) {
    case "string":
      return value.value || "(empty)";
    case "integer":
      return String(value.value);
    case "seed_random":
      return "Random seed";
    case "seed_fixed":
      return value.value;
    case "image_asset":
      return value.assetId;
    case "image_assets":
      return value.assetIds.length ? value.assetIds.join(" → ") : "No images";
    case "video_asset":
      return value.assetId;
    case "audio_asset":
      return value.assetId;
    case "video_assets":
      return value.assetIds.length ? value.assetIds.join(" → ") : "No videos";
    case "audio_assets":
      return value.assetIds.length ? value.assetIds.join(" → ") : "No audio";
  }
}

function formatDate(value: string): string {
  return new Date(value).toLocaleString();
}
