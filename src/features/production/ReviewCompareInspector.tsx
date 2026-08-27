import type {
  ReviewCompareCandidate,
  ReviewCompareContextSnapshot,
  ReviewCompareItem,
  ReviewCompareReferenceAsset,
} from "../../types/reviewProductivity";

export interface ReviewCompareInspectorProps {
  item: ReviewCompareItem;
  candidate?: ReviewCompareCandidate;
  /** The workspace already resolves this when the host has a richer adapter. */
  context?: ReviewCompareContextSnapshot;
}

export function resolveReviewCompareContext(
  item: ReviewCompareItem,
  candidate?: ReviewCompareCandidate,
): ReviewCompareContextSnapshot | undefined {
  const contexts = [
    candidate?.historicalContext,
    candidate?.contextSnapshot,
    candidate?.snapshot,
    item.historicalContext,
    item.contextSnapshot,
    item.snapshot,
    candidate?.context,
    item.context,
  ].filter((value): value is ReviewCompareContextSnapshot => Boolean(value));
  return contexts.find((value) => value.source !== "legacy")
    ?? contexts.find((value) => value.source === "legacy")
    ?? undefined;
}

function displayValue(value: unknown): string {
  if (value === undefined || value === null || value === "") return "—";
  if (typeof value === "string") return value;
  return JSON.stringify(value);
}

function referenceNames(values: Array<{ id: string; name: string }> | undefined): string {
  return values?.length ? values.map((value) => value.name || value.id).join("、") : "—";
}

function referenceAssetHash(sha256?: string): string {
  if (!sha256?.trim()) return "—";
  const value = sha256.trim();
  return value.length > 12 ? `${value.slice(0, 12)}…` : value;
}

export function ReviewCompareInspector({ item, candidate, context: suppliedContext }: ReviewCompareInspectorProps) {
  const context = suppliedContext ?? resolveReviewCompareContext(item, candidate);
  const currentName = context?.currentName ?? item.shotName ?? item.name;
  const isSnapshot = Boolean(context && context.source !== "legacy");
  const hasNegativePrompt = Boolean(context?.negativePrompt?.trim());

  return (
    <aside className="review-compare-inspector" aria-label="生产上下文检查器">
      <div className="review-compare-inspector-heading">
        <div><span className="section-label">只读上下文</span><h3>生产准备检查</h3></div>
        <span className={`review-compare-context-badge${isSnapshot ? " snapshot" : " legacy"}`}>{isSnapshot ? "历史快照" : "旧版任务"}</span>
      </div>

      {!isSnapshot && <p className="review-compare-legacy-note">旧版任务，无生产准备快照</p>}
      {context ? <ContextRows context={context} currentName={currentName} /> : <ContextRows context={{ prompt: candidate?.label }} currentName={currentName} />}
      <div className="review-compare-inspector-row review-compare-negative-prompt">
        <span>Negative Prompt</span>
        <strong>{hasNegativePrompt ? context?.negativePrompt : "当前 Workflow 未提供独立 Negative Prompt 输入"}</strong>
      </div>
    </aside>
  );
}

function ContextRows({ context, currentName }: { context: ReviewCompareContextSnapshot; currentName?: string }) {
  const historicalName = context.historicalName?.trim();
  const referenceSets = context.referenceSets?.map(({ id, name }) => ({ id, name }));
  const referenceAssets = context.referenceAssets?.map(({ id, name }) => ({ id, name }));
  const detailedReferenceAssets = context.referenceAssets
    ?? context.referenceSets?.flatMap((referenceSet) => referenceSet.assets ?? [])
    ?? [];
  return (
    <div className="review-compare-context-rows">
      {historicalName ? <InspectorRow label="历史名称" value={historicalName} /> : currentName ? <InspectorRow label="当前名称" value={currentName} /> : null}
      <InspectorRow label="Prompt" value={context.prompt ?? context.promptText} multiline />
      <InspectorRow label="Context" value={context.context} multiline />
      <InspectorRow label="Workflow" value={context.workflow ?? context.workflowName ?? context.workflowVersionId} />
      <InspectorRow label="Recipe" value={context.recipe ?? context.recipeName ?? context.recipeId} />
      <InspectorRow label="Context Hash" value={context.contextHash} mono />
      <InspectorRow label="Reference Sets" value={referenceNames(referenceSets)} />
      <InspectorRow label="Reference Assets" value={referenceNames(referenceAssets)} />
      <ReferenceAssetDetails assets={detailedReferenceAssets} />
      <InspectorRow label="Output Spec" value={context.outputSpec} />
      <InspectorRow label="Stage Input" value={context.stageInput} />
      <InspectorRow label="Readiness" value={context.readiness} />
    </div>
  );
}

function ReferenceAssetDetails({ assets }: { assets: ReviewCompareReferenceAsset[] }) {
  return (
    <div className="review-compare-reference-assets" aria-label="Reference Asset Inspector">
      <span>Reference Asset Inspector</span>
      {assets.length ? (
        <ul>
          {assets.map((asset) => {
            const hash = asset.sha256?.trim();
            return (
              <li key={`${asset.id}:${asset.ordinal ?? ""}`}>
                <strong>{asset.id || asset.name || "—"}</strong>
                <small>role：{asset.role || "—"} · ordinal：{asset.ordinal ?? "—"} · sha256：<code title={hash || undefined} aria-label={hash ? `完整 sha256：${hash}` : "缺少 sha256"}>{referenceAssetHash(hash)}</code></small>
              </li>
            );
          })}
        </ul>
      ) : <em>—</em>}
    </div>
  );
}

function InspectorRow({ label, value, multiline, mono }: { label: string; value: unknown; multiline?: boolean; mono?: boolean }) {
  return (
    <div className={`review-compare-inspector-row${multiline ? " multiline" : ""}`}>
      <span>{label}</span>
      <strong className={mono ? "review-compare-mono" : undefined}>{displayValue(value)}</strong>
    </div>
  );
}
