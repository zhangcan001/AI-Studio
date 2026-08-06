import { useEffect, useState } from "react";
import { getReusableDraft } from "../../services/tauriClient";
import type { DraftValue } from "../../types/generation";
import type { ReusableGenerationDraft, TaskDetail } from "../../types/history";
import { AssetCard } from "../assets/AssetCard";

const PROJECT_ID = "prj_default";

interface Props {
  detail: TaskDetail;
  loadingDraft: boolean;
  onBack: () => void;
  onLoadInputs: (draft: ReusableGenerationDraft) => void;
  onOpenAsset: (assetId: string) => void;
}

export function TaskHistoryDetail({ detail, loadingDraft: detailLoading, onBack, onLoadInputs, onOpenAsset }: Props) {
  const [draft, setDraft] = useState<ReusableGenerationDraft>();
  const [draftLoading, setDraftLoading] = useState(detailLoading);
  const [draftError, setDraftError] = useState<string>();

  useEffect(() => {
    if (!detail.reusableDraft.available) return;
    let active = true;
    setDraftLoading(true);
    setDraftError(undefined);
    void getReusableDraft(PROJECT_ID, detail.id)
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
  }, [detail.id, detail.reusableDraft.available]);

  return (
    <div className="task-detail-view">
      <button type="button" className="quiet-button back-button" onClick={onBack}>← Back to history</button>
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
          <p className="disabled-note">Missing image asset. Choose a replacement after loading inputs.</p>
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
              <AssetCard key={asset.id} asset={asset} onSelect={() => onOpenAsset(asset.id)} />
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
  }
}

function formatDate(value: string): string {
  return new Date(value).toLocaleString();
}
