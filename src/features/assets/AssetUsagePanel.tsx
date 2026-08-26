import { useEffect, useMemo, useState } from "react";
import { getAssetUsage } from "../../services/tauriClient";
import { toUserMessage } from "../../i18n/errorMessages";
import type { AssetUsageSummary, UsageRelation } from "../../types/consistency";

interface Props {
  projectId: string;
  assetId: string;
  assetName?: string;
}

type UsageBucket = "referenceSets" | "profiles" | "shots" | "legacyReferences" | "selectedKeyframes" | "productionHistory";

const bucketLabels: Record<UsageBucket, string> = {
  referenceSets: "ReferenceSet",
  profiles: "Profile",
  shots: "Shot",
  legacyReferences: "Legacy Anchor",
  selectedKeyframes: "选中关键帧",
  productionHistory: "历史生产引用",
};

function relationKind(item: UsageRelation): string {
  return `${item.relationType ?? ""} ${item.entityType ?? ""}`.toLocaleLowerCase();
}

const selectedKeyframeTerms = ["selected", "keyframe", "selected_output"];

function fallbackBucketItems(summary: AssetUsageSummary, bucket: UsageBucket): UsageRelation[] {
  const terms: Record<UsageBucket, string[]> = {
    referenceSets: ["reference_set", "referenceset"],
    profiles: ["profile"],
    shots: ["shot"],
    legacyReferences: ["anchor", "legacy"],
    selectedKeyframes: selectedKeyframeTerms,
    productionHistory: ["history", "production", "task", "review", "snapshot"],
  };
  return (summary.items ?? []).filter((item) => terms[bucket].some((term) => relationKind(item).includes(term)));
}

export function usageBucketItems(summary: AssetUsageSummary, bucket: UsageBucket): UsageRelation[] {
  const direct = summary[bucket];
  if (bucket === "shots" && Array.isArray(direct) && direct.length) {
    return direct.filter((item) => !selectedKeyframeTerms.some((term) => relationKind(item).includes(term)));
  }
  if (bucket !== "selectedKeyframes" && Array.isArray(direct) && direct.length) return direct;
  if (bucket === "selectedKeyframes") {
    const explicit = summary.selectedKeyframes ?? [];
    if (explicit.length) return explicit;
    const seen = new Set<string>();
    return [...(summary.items ?? []), ...(summary.shots ?? [])].filter((item, index) => {
      if (!selectedKeyframeTerms.some((term) => relationKind(item).includes(term))) return false;
      const key = `${relationKey(item, index)}:${item.relationType ?? ""}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }
  return fallbackBucketItems(summary, bucket);
}

function relationKey(item: UsageRelation, index: number): string {
  return `${item.entityType ?? "relation"}:${item.entityId ?? item.referenceSetId ?? index}`;
}

function relationTitle(item: UsageRelation): string {
  return item.displayName || item.detail || item.entityId || item.referenceSetId || "未命名关系";
}

function relationSubtitle(item: UsageRelation): string | undefined {
  const parts = [
    item.relationType,
    item.shotId ? `镜头 ${item.shotId}` : undefined,
    item.scopeType && item.scopeId ? `${item.scopeType} · ${item.scopeId}` : item.scopeType,
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : undefined;
}

export function AssetUsagePanel({ projectId, assetId, assetName }: Props) {
  const [summary, setSummary] = useState<AssetUsageSummary>();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string>();

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(undefined);
    setSummary(undefined);
    void getAssetUsage(projectId, assetId)
      .then((next) => { if (active) setSummary(next); })
      .catch((value: unknown) => { if (active) setError(toUserMessage(value)); })
      .finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [assetId, projectId]);

  const buckets = useMemo(() => summary
    ? (Object.keys(bucketLabels) as UsageBucket[]).map((bucket) => ({ bucket, items: usageBucketItems(summary, bucket) }))
    : [], [summary]);
  const visibleBuckets = buckets.filter(({ items }) => items.length);

  return (
    <section className="asset-usage-panel" aria-label={`${assetName ?? "素材"} 使用情况`} style={{ display: "grid", gap: 10, marginTop: 14, paddingTop: 14, borderTop: "1px solid var(--studio-border, rgba(255,255,255,.08))" }}>
      <div className="section-heading" style={{ marginBottom: 0 }}>
        <div><span className="section-label">Asset Usage</span><h3>使用情况</h3><p className="section-description">只在选中素材后读取当前项目的语义与生产引用。</p></div>
        {summary && <span className="status-pill">{summary.total} 条关系 · {summary.blockingCount} 个阻塞</span>}
      </div>
      {loading && <p className="disabled-note" role="status">正在加载使用情况…</p>}
      {error && <p className="error-message" role="alert">使用情况加载失败：{error}</p>}
      {summary && !visibleBuckets.length && <p className="empty-state">当前素材还没有 Profile、ReferenceSet、Shot 或历史引用。</p>}
      {summary && visibleBuckets.map(({ bucket, items }) => (
        <section key={bucket} aria-label={bucketLabels[bucket]} style={{ display: "grid", gap: 7 }}>
          <strong>{bucketLabels[bucket]}（{items.length}）</strong>
          <div style={{ display: "grid", gap: 6 }}>
            {items.slice(0, 10).map((item, index) => (
              <div key={relationKey(item, index)} style={{ display: "grid", gap: 2, padding: "7px 9px", border: "1px solid var(--studio-border, rgba(255,255,255,.08))", borderRadius: 7 }}>
                <span style={{ overflowWrap: "anywhere" }}>{relationTitle(item)}</span>
                {relationSubtitle(item) && <small style={{ color: "var(--studio-text-secondary, #9ca3af)" }}>{relationSubtitle(item)}</small>}
                {item.blocking && <small style={{ color: "var(--studio-danger, #f87171)" }}>当前关系会阻止删除</small>}
              </div>
            ))}
            {items.length > 10 && <small className="disabled-note">另有 {items.length - 10} 项关系。</small>}
          </div>
        </section>
      ))}
    </section>
  );
}

export { bucketLabels, fallbackBucketItems };
