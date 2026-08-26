import { useCallback, useEffect, useMemo, useState } from "react";
import {
  createReferenceSet,
  createReferenceSetFromAnchor,
  deleteReferenceSet,
  getReferenceSetDetail,
  getReferenceSetUsage,
  listConsistencyProfiles,
  listReferenceAnchors,
  listReferenceSets,
  updateReferenceSet,
} from "../../services/tauriClient";
import { toUserMessage } from "../../i18n/errorMessages";
import type { ReferenceAnchorView } from "../../types/referenceAnchor";
import type {
  ConsistencyProfileView,
  ReferenceSetDetailView,
  ReferenceSetDraft,
  ReferenceSetPurpose,
  ReferenceSetSummary,
  ReferenceSetUsageSummary,
  UsageRelation,
} from "../../types/consistency";
import { consistencyProfileTypes, referenceSetPurposes } from "../../types/consistency";
import { ReferenceSetEditor, purposeLabels } from "./ReferenceSetEditor";

interface Props {
  projectId: string;
}

const purposeFilters: Array<{ value: ReferenceSetPurpose | "ALL"; label: string }> = [
  { value: "ALL", label: "全部" },
  ...referenceSetPurposes.map((value) => ({ value, label: purposeLabels[value] })),
];

function relationKey(item: UsageRelation, index: number): string {
  return `${item.entityType ?? "relation"}:${item.entityId ?? item.referenceSetId ?? index}`;
}

function usageRelations(usage?: ReferenceSetUsageSummary): UsageRelation[] {
  if (!usage) return [];
  const buckets = [
    usage.profileDefaults,
    usage.costumes,
    usage.shotBindings,
    usage.scopeBindings,
    usage.owner ? [usage.owner] : [],
    usage.items,
  ];
  const result: UsageRelation[] = [];
  const seen = new Set<string>();
  buckets.flatMap((bucket) => bucket ?? []).forEach((item, index) => {
    const key = relationKey(item, index);
    if (seen.has(key)) return;
    seen.add(key);
    result.push(item);
  });
  return result;
}

function relationText(item: UsageRelation): string {
  return item.displayName || item.detail || item.entityId || item.referenceSetId || "未命名关系";
}

function relationSubtext(item: UsageRelation): string | undefined {
  const parts = [
    item.relationType,
    item.scopeType && item.scopeId ? `${item.scopeType} · ${item.scopeId}` : item.scopeType,
    item.shotId ? `镜头 ${item.shotId}` : undefined,
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : undefined;
}

function ReferenceSetUsagePanel({ referenceSet, usage, loading, error }: { referenceSet?: ReferenceSetSummary; usage?: ReferenceSetUsageSummary; loading: boolean; error?: string }) {
  const relations = usageRelations(usage);
  return (
    <aside className="reference-set-usage-panel" aria-label="参考集使用情况" style={{ display: "grid", gap: 10, minWidth: 0 }}>
      <div><span className="section-label">Usage</span><h3>使用情况</h3></div>
      {referenceSet && (
        <dl style={{ display: "grid", gridTemplateColumns: "auto minmax(0, 1fr)", gap: "6px 10px", margin: 0, fontSize: "0.78rem" }}>
          <dt>用途</dt><dd style={{ margin: 0 }}>{purposeLabels[referenceSet.purpose]}</dd>
          <dt>所有者</dt><dd style={{ margin: 0, overflowWrap: "anywhere" }}>{referenceSet.ownerProfileName ?? referenceSet.ownerProfileId ?? "未设置"}</dd>
          <dt>图片数</dt><dd style={{ margin: 0 }}>{referenceSet.itemCount ?? referenceSet.imageCount ?? "—"}</dd>
        </dl>
      )}
      {loading && <p className="disabled-note" role="status">正在加载参考集使用情况…</p>}
      {error && <p className="error-message" role="alert">{error}</p>}
      {usage && (
        <>
          <div style={{ display: "grid", gridTemplateColumns: "repeat(2, minmax(0, 1fr))", gap: 7 }}>
            <div className="status-pill"><strong>{usage.total}</strong><small> 条关系</small></div>
            <div className="status-pill"><strong>{usage.blockingCount}</strong><small> 个阻塞</small></div>
          </div>
          <div style={{ display: "grid", gap: 8 }}>
            {relations.slice(0, 10).map((item, index) => (
              <div key={relationKey(item, index)} style={{ padding: "8px 9px", border: "1px solid var(--studio-border, rgba(255,255,255,.08))", borderRadius: 7 }}>
                <strong style={{ display: "block", overflowWrap: "anywhere" }}>{relationText(item)}</strong>
                {relationSubtext(item) && <small style={{ display: "block", color: "var(--studio-text-secondary, #9ca3af)" }}>{relationSubtext(item)}</small>}
                {item.blocking && <small style={{ display: "block", color: "var(--studio-danger, #f87171)" }}>正在使用，删除前需解除关系</small>}
              </div>
            ))}
            {relations.length > 10 && <p className="disabled-note">另有 {relations.length - 10} 项关系。</p>}
            {!relations.length && <p className="empty-state">当前还没有可读的使用关系。</p>}
          </div>
        </>
      )}
      {!usage && !loading && !error && <p className="empty-state">选中参考集后显示使用位置。</p>}
    </aside>
  );
}

export function ReferenceSetLibrary({ projectId }: Props) {
  const [purpose, setPurpose] = useState<ReferenceSetPurpose | "ALL">("ALL");
  const [sets, setSets] = useState<ReferenceSetSummary[]>([]);
  const [selectedSetId, setSelectedSetId] = useState<string>();
  const [detail, setDetail] = useState<ReferenceSetDetailView>();
  const [usage, setUsage] = useState<ReferenceSetUsageSummary>();
  const [ownerProfiles, setOwnerProfiles] = useState<ConsistencyProfileView[]>([]);
  const [creating, setCreating] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [listLoading, setListLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [keyword, setKeyword] = useState("");
  const [error, setError] = useState<string>();
  const [detailError, setDetailError] = useState<string>();
  const [usageError, setUsageError] = useState<string>();
  const [editorNonce, setEditorNonce] = useState(0);
  const [conversionOpen, setConversionOpen] = useState(false);
  const [anchors, setAnchors] = useState<ReferenceAnchorView[]>([]);
  const [anchorId, setAnchorId] = useState("");
  const [conversionName, setConversionName] = useState("");
  const [conversionBusy, setConversionBusy] = useState(false);
  const [conversionError, setConversionError] = useState<string>();

  const refreshSets = useCallback(async (preferredId?: string) => {
    setListLoading(true);
    setError(undefined);
    try {
      const next = await listReferenceSets(projectId, purpose === "ALL" ? undefined : purpose);
      setSets(next);
      setSelectedSetId((current) => {
        const candidate = preferredId ?? current;
        return candidate && next.some((item) => item.id === candidate) ? candidate : next[0]?.id;
      });
    } catch (value: unknown) {
      setError(toUserMessage(value));
      setSets([]);
      setSelectedSetId(undefined);
    } finally {
      setListLoading(false);
    }
  }, [projectId, purpose]);

  useEffect(() => {
    setCreating(false);
    setSelectedSetId(undefined);
    setDetail(undefined);
    setUsage(undefined);
    setDirty(false);
    setKeyword("");
    void refreshSets();
  }, [projectId, purpose, refreshSets]);

  useEffect(() => {
    let active = true;
    void Promise.all(consistencyProfileTypes.map((profileType) => listConsistencyProfiles(projectId, profileType))).then((groups) => {
      if (!active) return;
      const byId = new Map<string, ConsistencyProfileView>();
      groups.flat().forEach((profile) => byId.set(profile.id, profile));
      setOwnerProfiles([...byId.values()]);
    }).catch((value: unknown) => {
      if (active) setError(toUserMessage(value));
    });
    return () => { active = false; };
  }, [projectId]);

  useEffect(() => {
    if (creating || !selectedSetId) {
      setDetail(undefined);
      setUsage(undefined);
      setDetailLoading(false);
      return;
    }
    let active = true;
    setDetailLoading(true);
    setDetailError(undefined);
    setUsageError(undefined);
    void getReferenceSetDetail(projectId, selectedSetId)
      .then((next) => { if (active) setDetail(next); })
      .catch((value: unknown) => { if (active) setDetailError(toUserMessage(value)); })
      .finally(() => { if (active) setDetailLoading(false); });
    void getReferenceSetUsage(projectId, selectedSetId)
      .then((next) => { if (active) setUsage(next); })
      .catch((value: unknown) => { if (active) setUsageError(toUserMessage(value)); });
    return () => { active = false; };
  }, [creating, projectId, selectedSetId]);

  const visibleSets = useMemo(() => {
    const normalized = keyword.trim().toLocaleLowerCase();
    if (!normalized) return sets;
    return sets.filter((item) => item.name.toLocaleLowerCase().includes(normalized) || item.description.toLocaleLowerCase().includes(normalized));
  }, [keyword, sets]);

  const currentSet = detail?.referenceSet ?? sets.find((item) => item.id === selectedSetId);
  const usageRelationsList = usageRelations(usage);
  const usageBlocked = Boolean(usage && (usage.blockingCount > 0 || usageRelationsList.some((item) => item.blocking)));

  function canLeaveEditor(): boolean {
    if (!dirty) return true;
    const confirmed = window.confirm("当前参考集有未保存的修改，确定放弃并切换吗？");
    if (confirmed) setDirty(false);
    return confirmed;
  }

  function startCreate() {
    if (!canLeaveEditor()) return;
    setCreating(true);
    setSelectedSetId(undefined);
    setDetail(undefined);
    setUsage(undefined);
    setDetailError(undefined);
    setUsageError(undefined);
    setEditorNonce((value) => value + 1);
  }

  function selectSet(id: string) {
    if (id === selectedSetId && !creating) return;
    if (!canLeaveEditor()) return;
    setCreating(false);
    setSelectedSetId(id);
    setDetail(undefined);
    setUsage(undefined);
    setDetailError(undefined);
    setUsageError(undefined);
    setEditorNonce((value) => value + 1);
  }

  function changePurpose(next: ReferenceSetPurpose | "ALL") {
    if (next === purpose || !canLeaveEditor()) return;
    setPurpose(next);
  }

  async function saveSet(draft: ReferenceSetDraft) {
    setSaving(true);
    setError(undefined);
    try {
      const request = { projectId, ...draft };
      const saved = currentSet
        ? await updateReferenceSet({ ...request, referenceSetId: currentSet.id })
        : await createReferenceSet(request);
      setCreating(false);
      setSelectedSetId(saved.id);
      setDetail(undefined);
      setUsage(undefined);
      setDirty(false);
      setEditorNonce((value) => value + 1);
      await refreshSets(saved.id);
    } catch (value: unknown) {
      throw value;
    } finally {
      setSaving(false);
    }
  }

  async function removeSet() {
    const referenceSet = currentSet;
    if (!referenceSet) return;
    setSaving(true);
    setError(undefined);
    try {
      const latestUsage = await getReferenceSetUsage(projectId, referenceSet.id);
      setUsage(latestUsage);
      const latestRelations = usageRelations(latestUsage);
      if (latestUsage.blockingCount > 0 || latestRelations.some((item) => item.blocking)) {
        setError(`该参考集正在被使用，无法删除。${latestRelations.filter((item) => item.blocking).slice(0, 10).map(relationText).join("；")}`);
        return;
      }
      if (!window.confirm(`确定删除参考集“${referenceSet.name}”吗？素材本身不会被删除。`)) return;
      await deleteReferenceSet(projectId, referenceSet.id);
      setSelectedSetId(undefined);
      setDetail(undefined);
      setUsage(undefined);
      setEditorNonce((value) => value + 1);
      await refreshSets();
    } catch (value: unknown) {
      setError(toUserMessage(value));
    } finally {
      setSaving(false);
    }
  }

  async function openConversion() {
    setConversionOpen(true);
    setConversionError(undefined);
    setAnchorId("");
    setConversionName("");
    try {
      setAnchors(await listReferenceAnchors(projectId));
    } catch (value: unknown) {
      setConversionError(toUserMessage(value));
      setAnchors([]);
    }
  }

  async function convertAnchor() {
    const normalizedName = conversionName.trim();
    if (!anchorId) {
      setConversionError("请选择要转换的旧参考锚点。");
      return;
    }
    if (!normalizedName) {
      setConversionError("请输入新参考集名称。");
      return;
    }
    setConversionBusy(true);
    setConversionError(undefined);
    try {
      const created = await createReferenceSetFromAnchor(projectId, anchorId, normalizedName);
      setConversionOpen(false);
      setSelectedSetId(created.id);
      setCreating(false);
      setDetail(undefined);
      setUsage(undefined);
      await refreshSets(created.id);
    } catch (value: unknown) {
      setConversionError(toUserMessage(value));
    } finally {
      setConversionBusy(false);
    }
  }

  return (
    <section className="workspace-panel reference-set-library" aria-label="参考集库" style={{ display: "grid", gap: 14, minWidth: 0 }}>
      <div className="section-heading workspace-heading" style={{ alignItems: "flex-start", marginBottom: 0 }}>
        <div>
          <span className="section-label">ReferenceSet</span>
          <h2>参考集</h2>
          <p className="section-description">管理可复用的有序图片集合；旧版参考锚点仍保留，不会被自动转换或删除。</p>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", justifyContent: "flex-end" }}>
          <button type="button" className="quiet-button" onClick={() => void openConversion()} disabled={listLoading || saving}>从旧参考锚点创建</button>
          <button type="button" className="primary-action" onClick={startCreate} disabled={listLoading || saving}>新建参考集</button>
        </div>
      </div>

      <div className="filter-row" role="tablist" aria-label="参考集用途">
        {purposeFilters.map((item) => (
          <button key={item.value} type="button" role="tab" aria-selected={purpose === item.value} className={purpose === item.value ? "filter-button filter-button-active" : "filter-button"} onClick={() => changePurpose(item.value)}>{item.label}</button>
        ))}
      </div>
      {error && <p className="error-message" role="alert">{error}</p>}
      <div style={{ display: "grid", gridTemplateColumns: "minmax(210px, .72fr) minmax(0, 1.65fr) minmax(210px, .8fr)", gap: 12, alignItems: "start" }}>
        <aside className="reference-set-list" aria-label="参考集列表" style={{ display: "grid", gap: 8, minWidth: 0 }}>
          <label className="field-control"><span>搜索参考集</span><input value={keyword} onChange={(event) => setKeyword(event.target.value)} placeholder="按名称或说明搜索" /></label>
          {listLoading && <p className="disabled-note" role="status">正在加载参考集…</p>}
          {!listLoading && !visibleSets.length && (
            <div className="empty-state" style={{ display: "grid", gap: 8, padding: 12, border: "1px dashed var(--studio-border-strong, rgba(255,255,255,.12))", borderRadius: 8 }}>
              <strong>{keyword.trim() ? "没有符合条件的参考集。" : "当前项目还没有参考集。"}</strong>
              {!keyword.trim() && <button type="button" onClick={startCreate}>新建参考集</button>}
            </div>
          )}
          {visibleSets.map((item) => (
            <button key={item.id} type="button" className={item.id === selectedSetId && !creating ? "filter-button filter-button-active" : "filter-button"} onClick={() => selectSet(item.id)} style={{ display: "grid", gap: 3, minWidth: 0, textAlign: "left" }}>
              <strong style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{item.name}</strong>
              <small style={{ color: "var(--studio-text-secondary, #9ca3af)" }}>{purposeLabels[item.purpose]} · {item.itemCount ?? item.imageCount ?? "—"} 张图片</small>
              <small style={{ color: "var(--studio-text-muted, #6b7280)", overflow: "hidden", textOverflow: "ellipsis" }}>{item.ownerProfileName ?? "未设置所有者"}</small>
            </button>
          ))}
        </aside>

        <div style={{ minWidth: 0 }}>
          {(creating || currentSet) && (
            <ReferenceSetEditor
              key={`${creating ? "new" : currentSet?.id ?? "empty"}:${detail ? "detail" : "summary"}:${editorNonce}`}
              projectId={projectId}
              referenceSet={creating ? undefined : currentSet}
              detail={creating ? undefined : detail}
              ownerProfiles={ownerProfiles}
              onSave={saveSet}
              onCancel={() => { if (canLeaveEditor()) { setCreating(false); setSelectedSetId(undefined); setDetail(undefined); } }}
              onDelete={removeSet}
              onDirtyChange={setDirty}
              busy={saving || detailLoading}
              deleteBlocked={usageBlocked}
              error={detailError}
            />
          )}
          {!creating && !currentSet && detailLoading && <p className="empty-state" role="status">正在加载参考集详情…</p>}
        </div>

        <div style={{ display: "grid", gap: 10, minWidth: 0 }}>
          <ReferenceSetUsagePanel referenceSet={currentSet} usage={usage} loading={detailLoading} error={usageError} />
          {usageBlocked && <p className="error-message" role="alert">该参考集正在被使用，删除按钮已禁用。</p>}
          {usage && !usageBlocked && <p className="disabled-note">当前没有阻塞关系，可以在确认后删除。</p>}
        </div>
      </div>

      {conversionOpen && (
        <div className="asset-preview-backdrop" role="presentation" onMouseDown={() => !conversionBusy && setConversionOpen(false)}>
          <section className="asset-preview-panel" role="dialog" aria-modal="true" aria-label="从旧参考锚点创建参考集" onMouseDown={(event) => event.stopPropagation()} style={{ display: "grid", gap: 12 }}>
            <div className="section-heading" style={{ marginBottom: 0 }}><div><span className="section-label">Legacy Anchor</span><h3>从旧参考锚点创建</h3><p className="section-description">这是显式转换；原参考锚点和顺序会保持不变。</p></div><button type="button" className="quiet-button" onClick={() => setConversionOpen(false)} disabled={conversionBusy}>关闭</button></div>
            <label className="field-control"><span>旧参考锚点</span><select value={anchorId} onChange={(event) => setAnchorId(event.target.value)} disabled={conversionBusy}><option value="">请选择</option>{anchors.map((anchor) => <option key={anchor.id} value={anchor.id}>{anchor.name} · {anchor.assets.length} 张图片</option>)}</select></label>
            <label className="field-control"><span>新参考集名称</span><input value={conversionName} onChange={(event) => setConversionName(event.target.value)} placeholder="例如：主角参考集" disabled={conversionBusy} /></label>
            {conversionError && <p className="error-message" role="alert">{conversionError}</p>}
            <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}><button type="button" className="quiet-button" onClick={() => setConversionOpen(false)} disabled={conversionBusy}>取消</button><button type="button" onClick={() => void convertAnchor()} disabled={conversionBusy || !anchorId || !conversionName.trim()}>{conversionBusy ? "正在创建…" : "确认创建"}</button></div>
          </section>
        </div>
      )}
    </section>
  );
}

export { purposeFilters, usageRelations };
