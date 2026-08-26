import { useCallback, useEffect, useMemo, useState } from "react";
import {
  createCharacterProfile,
  createCostumeVariant,
  createPropProfile,
  createSceneProfile,
  createStyleProfile,
  deleteConsistencyProfile,
  deleteCostumeVariant,
  getConsistencyProfile,
  getProfileUsage,
  listConsistencyProfiles,
  listCostumeVariants,
  listReferenceSets,
  updateCharacterProfile,
  updateCostumeVariant,
  updatePropProfile,
  updateSceneProfile,
  updateStyleProfile,
} from "../../services/tauriClient";
import { toUserMessage } from "../../i18n/errorMessages";
import type {
  ConsistencyProfileDraft,
  ConsistencyProfileView,
  CostumeVariantRequest,
  CostumeVariantUpdateRequest,
  CostumeVariantView,
  ProfileType,
  ProfileUsageSummary,
  ReferenceSetSummary,
  UsageRelation,
} from "../../types/consistency";
import { consistencyProfileTypes } from "../../types/consistency";
import { ConsistencyProfileEditor } from "./ConsistencyProfileEditor";

interface Props {
  projectId: string;
}

const profileLabels: Record<ProfileType, string> = {
  CHARACTER: "角色",
  SCENE: "场景",
  PROP: "道具",
  STYLE: "风格",
};

function relationKey(item: UsageRelation, index: number): string {
  return `${item.entityType ?? "relation"}:${item.entityId ?? item.referenceSetId ?? index}`;
}

function usageRelations(usage?: ProfileUsageSummary): UsageRelation[] {
  if (!usage) return [];
  const buckets = [
    usage.shotBindings,
    usage.scopeBindings,
    usage.referenceSets,
    usage.defaultStyleProfiles,
    usage.costumeVariants,
    usage.relatedProfiles,
    usage.items,
  ];
  const result: UsageRelation[] = [];
  const seen = new Set<string>();
  buckets.flatMap((bucket) => bucket ?? []).forEach((item, index) => {
    const key = relationKey(item, index);
    if (!seen.has(key)) {
      seen.add(key);
      result.push(item);
    }
  });
  return result;
}

function usageBlockers(usage?: ProfileUsageSummary): UsageRelation[] {
  return usageRelations(usage).filter((item) => item.blocking);
}

function usageRelationText(item: UsageRelation): string {
  return item.displayName || item.detail || item.entityId || item.referenceSetId || "未命名关系";
}

function usageRelationSubtext(item: UsageRelation): string | undefined {
  const parts = [
    item.relationType,
    item.scopeType && item.scopeId ? `${item.scopeType} · ${item.scopeId}` : item.scopeType,
    item.shotId ? `镜头 ${item.shotId}` : undefined,
  ].filter(Boolean);
  return parts.length ? parts.join(" · ") : undefined;
}

function ProfileUsagePanel({ profile, usage, loading, error }: { profile?: ConsistencyProfileView; usage?: ProfileUsageSummary; loading: boolean; error?: string }) {
  const relations = usageRelations(usage);
  return (
    <aside className="consistency-usage-panel" aria-label="档案使用情况" style={{ display: "grid", gap: 10, minWidth: 0 }}>
      <div>
        <span className="section-label">Usage</span>
        <h3>使用情况</h3>
      </div>
      {profile && (
        <dl style={{ display: "grid", gridTemplateColumns: "auto minmax(0, 1fr)", gap: "6px 10px", margin: 0, fontSize: "0.78rem" }}>
          <dt>类型</dt><dd style={{ margin: 0 }}>{profileLabels[profile.profileType]}</dd>
          <dt>Revision</dt><dd style={{ margin: 0, overflowWrap: "anywhere" }}>{profile.activeRevisionId ?? "尚未生成"}</dd>
          <dt>默认参考集</dt><dd style={{ margin: 0, overflowWrap: "anywhere" }}>{profile.defaultReferenceSetId ?? "未设置"}</dd>
        </dl>
      )}
      {loading && <p className="disabled-note" role="status">正在加载档案使用情况…</p>}
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
                <strong style={{ display: "block", overflowWrap: "anywhere" }}>{usageRelationText(item)}</strong>
                {usageRelationSubtext(item) && <small style={{ display: "block", color: "var(--studio-text-secondary, #9ca3af)" }}>{usageRelationSubtext(item)}</small>}
                {item.blocking && <small style={{ display: "block", color: "var(--studio-danger, #f87171)" }}>正在使用，删除前需解除关系</small>}
              </div>
            ))}
            {relations.length > 10 && <p className="disabled-note">另有 {relations.length - 10} 项关系。</p>}
            {!relations.length && <p className="empty-state">当前还没有可读的使用关系。</p>}
          </div>
        </>
      )}
      {!usage && !loading && !error && <p className="empty-state">选中档案后显示使用位置。</p>}
    </aside>
  );
}

export function ConsistencyProfileLibrary({ projectId }: Props) {
  const [profileType, setProfileType] = useState<ProfileType>("CHARACTER");
  const [profiles, setProfiles] = useState<ConsistencyProfileView[]>([]);
  const [selectedProfileId, setSelectedProfileId] = useState<string>();
  const [selectedProfile, setSelectedProfile] = useState<ConsistencyProfileView>();
  const [costumes, setCostumes] = useState<CostumeVariantView[]>([]);
  const [referenceSets, setReferenceSets] = useState<ReferenceSetSummary[]>([]);
  const [styleProfiles, setStyleProfiles] = useState<ConsistencyProfileView[]>([]);
  const [profileUsage, setProfileUsage] = useState<ProfileUsageSummary>();
  const [keyword, setKeyword] = useState("");
  const [creating, setCreating] = useState(false);
  const [listLoading, setListLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [error, setError] = useState<string>();
  const [detailError, setDetailError] = useState<string>();
  const [usageError, setUsageError] = useState<string>();
  const [editorNonce, setEditorNonce] = useState(0);

  const refreshProfiles = useCallback(async (preferredId?: string) => {
    setListLoading(true);
    setError(undefined);
    try {
      const next = await listConsistencyProfiles(projectId, profileType);
      setProfiles(next);
      setSelectedProfileId((current) => {
        const candidate = preferredId ?? current;
        return candidate && next.some((item) => item.id === candidate) ? candidate : next[0]?.id;
      });
    } catch (value: unknown) {
      setError(toUserMessage(value));
      setProfiles([]);
      setSelectedProfileId(undefined);
    } finally {
      setListLoading(false);
    }
  }, [profileType, projectId]);

  useEffect(() => {
    setCreating(false);
    setSelectedProfile(undefined);
    setCostumes([]);
    setProfileUsage(undefined);
    setDirty(false);
    setKeyword("");
    void refreshProfiles();
  }, [profileType, projectId, refreshProfiles]);

  useEffect(() => {
    let active = true;
    void Promise.all([
      listReferenceSets(projectId),
      listConsistencyProfiles(projectId, "STYLE"),
    ]).then(([sets, styles]) => {
      if (!active) return;
      setReferenceSets(sets);
      setStyleProfiles(styles);
    }).catch((value: unknown) => {
      if (active) setError(toUserMessage(value));
    });
    return () => { active = false; };
  }, [projectId]);

  useEffect(() => {
    if (creating || !selectedProfileId) {
      setSelectedProfile(undefined);
      setCostumes([]);
      setProfileUsage(undefined);
      setDetailLoading(false);
      return;
    }
    let active = true;
    setDetailLoading(true);
    setDetailError(undefined);
    setUsageError(undefined);
    void getConsistencyProfile(projectId, profileType, selectedProfileId)
      .then((next) => {
        if (active) setSelectedProfile(next);
      })
      .catch((value: unknown) => {
        if (active) setDetailError(toUserMessage(value));
      })
      .finally(() => {
        if (active) setDetailLoading(false);
      });
    void getProfileUsage(projectId, profileType, selectedProfileId)
      .then((next) => { if (active) setProfileUsage(next); })
      .catch((value: unknown) => { if (active) setUsageError(toUserMessage(value)); });
    if (profileType === "CHARACTER") {
      void listCostumeVariants(projectId, selectedProfileId)
        .then((next) => { if (active) setCostumes(next); })
        .catch((value: unknown) => { if (active) setDetailError(toUserMessage(value)); });
    } else {
      setCostumes([]);
    }
    return () => { active = false; };
  }, [creating, profileType, projectId, selectedProfileId]);

  const visibleProfiles = useMemo(() => {
    const normalized = keyword.trim().toLocaleLowerCase();
    if (!normalized) return profiles;
    return profiles.filter((item) => item.name.toLocaleLowerCase().includes(normalized));
  }, [keyword, profiles]);

  const currentProfile = selectedProfile ?? profiles.find((item) => item.id === selectedProfileId);
  const blockers = usageBlockers(profileUsage);
  const profileUsageBlocked = Boolean(profileUsage && (profileUsage.blockingCount > 0 || blockers.length > 0));

  function canLeaveEditor(): boolean {
    if (!dirty) return true;
    const confirmed = window.confirm("当前档案有未保存的修改，确定放弃并切换吗？");
    if (confirmed) setDirty(false);
    return confirmed;
  }

  function startCreate() {
    if (!canLeaveEditor()) return;
    setCreating(true);
    setSelectedProfileId(undefined);
    setSelectedProfile(undefined);
    setProfileUsage(undefined);
    setCostumes([]);
    setDetailError(undefined);
    setUsageError(undefined);
    setEditorNonce((value) => value + 1);
  }

  function selectProfile(id: string) {
    if (id === selectedProfileId && !creating) return;
    if (!canLeaveEditor()) return;
    setCreating(false);
    setSelectedProfileId(id);
    setSelectedProfile(undefined);
    setProfileUsage(undefined);
    setCostumes([]);
    setDetailError(undefined);
    setUsageError(undefined);
    setEditorNonce((value) => value + 1);
  }

  function changeProfileType(next: ProfileType) {
    if (next === profileType || !canLeaveEditor()) return;
    setProfileType(next);
  }

  async function saveProfile(draft: ConsistencyProfileDraft) {
    setSaving(true);
    setError(undefined);
    try {
      let saved: ConsistencyProfileView;
      if (draft.profileType === "CHARACTER") {
        const request = {
          projectId,
          name: draft.name,
          description: draft.description,
          canonicalPrompt: draft.canonicalPrompt,
          negativePrompt: draft.negativePrompt,
          defaultStyleProfileId: draft.defaultStyleProfileId || null,
          defaultReferenceSetId: draft.defaultReferenceSetId || null,
          metadataJson: draft.metadataJson,
        };
        saved = currentProfile
          ? await updateCharacterProfile({ ...request, profileId: currentProfile.id })
          : await createCharacterProfile(request);
      } else if (draft.profileType === "SCENE") {
        const request = {
          projectId,
          name: draft.name,
          description: draft.description,
          environmentPrompt: draft.environmentPrompt,
          lightingPrompt: draft.lightingPrompt || null,
          negativePrompt: draft.negativePrompt || null,
          defaultStyleProfileId: draft.defaultStyleProfileId || null,
          defaultReferenceSetId: draft.defaultReferenceSetId || null,
        };
        saved = currentProfile
          ? await updateSceneProfile({ ...request, profileId: currentProfile.id })
          : await createSceneProfile(request);
      } else if (draft.profileType === "PROP") {
        const request = {
          projectId,
          name: draft.name,
          description: draft.description,
          canonicalPrompt: draft.canonicalPrompt,
          materialPrompt: draft.materialPrompt || null,
          scalePrompt: draft.scalePrompt || null,
          defaultReferenceSetId: draft.defaultReferenceSetId || null,
        };
        saved = currentProfile
          ? await updatePropProfile({ ...request, profileId: currentProfile.id })
          : await createPropProfile(request);
      } else {
        const request = {
          projectId,
          name: draft.name,
          stylePrompt: draft.stylePrompt,
          colorPrompt: draft.colorPrompt || null,
          linePrompt: draft.linePrompt || null,
          negativePrompt: draft.negativePrompt || null,
          outputNotes: draft.outputNotes || null,
        };
        saved = currentProfile
          ? await updateStyleProfile({ ...request, profileId: currentProfile.id })
          : await createStyleProfile(request);
      }
      setSelectedProfile(saved);
      setCreating(false);
      setSelectedProfileId(saved.id);
      setDirty(false);
      setEditorNonce((value) => value + 1);
      await refreshProfiles(saved.id);
      const nextUsage = await getProfileUsage(projectId, saved.profileType, saved.id).catch(() => undefined);
      setProfileUsage(nextUsage);
      if (saved.profileType === "CHARACTER") {
        const nextCostumes = await listCostumeVariants(projectId, saved.id).catch(() => []);
        setCostumes(nextCostumes);
      }
    } catch (value: unknown) {
      throw value;
    } finally {
      setSaving(false);
    }
  }

  async function saveCostume(request: CostumeVariantRequest | CostumeVariantUpdateRequest) {
    if (request.isDefault) {
      const keepId = "costumeVariantId" in request ? request.costumeVariantId : undefined;
      for (const variant of costumes.filter((item) => item.isDefault && item.id !== keepId)) {
        await updateCostumeVariant({
          projectId,
          costumeVariantId: variant.id,
          name: variant.name,
          promptFragment: variant.promptFragment,
          referenceSetId: variant.referenceSetId ?? null,
          isDefault: false,
          ordinal: variant.ordinal,
        });
      }
    }
    if ("costumeVariantId" in request) await updateCostumeVariant(request);
    else await createCostumeVariant(request);
    if (selectedProfileId) setCostumes(await listCostumeVariants(projectId, selectedProfileId));
  }

  async function removeCostume(variant: CostumeVariantView) {
    await deleteCostumeVariant(projectId, variant.id);
    if (selectedProfileId) setCostumes(await listCostumeVariants(projectId, selectedProfileId));
  }

  async function removeProfile() {
    const profile = currentProfile;
    if (!profile) return;
    setError(undefined);
    setSaving(true);
    try {
      const latestUsage = await getProfileUsage(projectId, profile.profileType, profile.id);
      setProfileUsage(latestUsage);
      const latestBlockers = usageBlockers(latestUsage);
      if (latestUsage.blockingCount > 0 || latestBlockers.length > 0) {
        setError(`该档案正在被使用，无法删除。${latestBlockers.slice(0, 10).map(usageRelationText).join("；")}`);
        return;
      }
      if (!window.confirm(`确定删除档案“${profile.name}”吗？`)) return;
      await deleteConsistencyProfile(projectId, profile.profileType, profile.id);
      setCreating(false);
      setSelectedProfile(undefined);
      setSelectedProfileId(undefined);
      setProfileUsage(undefined);
      setEditorNonce((value) => value + 1);
      await refreshProfiles();
    } catch (value: unknown) {
      setError(toUserMessage(value));
    } finally {
      setSaving(false);
    }
  }

  const ownerHint = currentProfile?.defaultReferenceSetId
    ? referenceSets.find((set) => set.id === currentProfile.defaultReferenceSetId)
    : undefined;

  return (
    <section className="workspace-panel consistency-profile-library" aria-label="档案库" style={{ display: "grid", gap: 14, minWidth: 0 }}>
      <div className="section-heading workspace-heading" style={{ alignItems: "flex-start", marginBottom: 0 }}>
        <div>
          <span className="section-label">Consistency</span>
          <h2>档案库</h2>
          <p className="section-description">管理角色、场景、道具和风格 Profile；这里只保存语义与关系，不复制素材文件。</p>
        </div>
        <button type="button" className="primary-action" onClick={startCreate} disabled={listLoading || saving}>新建{profileLabels[profileType]}档案</button>
      </div>

      <div className="filter-row" role="tablist" aria-label="档案类型">
        {consistencyProfileTypes.map((value) => (
          <button key={value} type="button" role="tab" aria-selected={profileType === value} className={profileType === value ? "filter-button filter-button-active" : "filter-button"} onClick={() => changeProfileType(value)}>
            {profileLabels[value]}
          </button>
        ))}
      </div>

      {error && <p className="error-message" role="alert">{error}</p>}
      <div style={{ display: "grid", gridTemplateColumns: "minmax(190px, .72fr) minmax(0, 1.65fr) minmax(210px, .8fr)", gap: 12, alignItems: "start" }}>
        <aside className="consistency-profile-list" aria-label={`${profileLabels[profileType]}档案列表`} style={{ display: "grid", gap: 8, minWidth: 0 }}>
          <label className="field-control"><span>按名称筛选</span><input value={keyword} onChange={(event) => setKeyword(event.target.value)} placeholder={`搜索${profileLabels[profileType]}名称`} /></label>
          {listLoading && <p className="disabled-note" role="status">正在加载档案…</p>}
          {!listLoading && !visibleProfiles.length && (
            <div className="empty-state" style={{ display: "grid", gap: 8, padding: 12, border: "1px dashed var(--studio-border-strong, rgba(255,255,255,.12))", borderRadius: 8 }}>
              <strong>{keyword.trim() ? "没有符合条件的档案。" : `当前项目还没有${profileLabels[profileType]}档案。`}</strong>
              {!keyword.trim() && <button type="button" onClick={startCreate}>新建{profileLabels[profileType]}档案</button>}
            </div>
          )}
          {visibleProfiles.map((item) => (
            <button key={item.id} type="button" className={item.id === selectedProfileId && !creating ? "filter-button filter-button-active" : "filter-button"} onClick={() => selectProfile(item.id)} style={{ display: "grid", gap: 3, minWidth: 0, textAlign: "left" }}>
              <strong style={{ overflow: "hidden", textOverflow: "ellipsis" }}>{item.name}</strong>
              <small style={{ color: "var(--studio-text-secondary, #9ca3af)", overflow: "hidden", textOverflow: "ellipsis" }}>{item.description || "暂无描述"}</small>
            </button>
          ))}
        </aside>

        <div style={{ minWidth: 0 }}>
          {(creating || currentProfile) && (
            <ConsistencyProfileEditor
              key={`${profileType}:${creating ? "new" : currentProfile?.id ?? "empty"}:${selectedProfile ? "detail" : "summary"}:${editorNonce}`}
              profileType={profileType}
              profile={creating ? undefined : currentProfile}
              costumes={costumes}
              referenceSets={referenceSets}
              styleProfiles={styleProfiles}
              onSave={saveProfile}
              onCancel={() => { if (canLeaveEditor()) { setCreating(false); setSelectedProfileId(undefined); setSelectedProfile(undefined); } }}
              onDelete={removeProfile}
              onCostumeCreate={saveCostume}
              onCostumeUpdate={saveCostume}
              onCostumeDelete={removeCostume}
              onDirtyChange={setDirty}
              busy={saving || detailLoading}
              deleteBlocked={profileUsageBlocked}
              error={detailError}
            />
          )}
          {!creating && !currentProfile && detailLoading && <p className="empty-state" role="status">正在加载档案详情…</p>}
        </div>

        <div style={{ display: "grid", gap: 10, minWidth: 0 }}>
          {ownerHint && <p className="disabled-note">默认参考集：{ownerHint.name}</p>}
          <ProfileUsagePanel profile={currentProfile} usage={profileUsage} loading={detailLoading} error={usageError} />
          {profileUsageBlocked && <p className="error-message" role="alert">该档案正在被使用，删除按钮已禁用。</p>}
          {profileUsage && !profileUsageBlocked && <p className="disabled-note">当前没有阻塞关系，可以在确认后删除。</p>}
        </div>
      </div>
    </section>
  );
}

export { profileLabels, usageRelations, usageBlockers };
