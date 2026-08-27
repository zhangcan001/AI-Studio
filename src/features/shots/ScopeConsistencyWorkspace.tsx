import { useEffect, useMemo, useState } from "react";
import { ConsistencyBindingEditor } from "./ConsistencyBindingEditor";
import type {
  ConsistencyBindingPack,
  ConsistencyBindingReplaceInput,
  ConsistencyContextPreview,
  ConsistencyCostumeOption,
  ConsistencyProfileOption,
  ConsistencyReferenceSetOption,
  ConsistencyScopeOption,
  ConsistencyScopeRef,
  ConsistencyScopeType,
} from "../../types/consistencyBindings";
import type { ShotStage } from "../../types/shot";
import { roleLabel, scopeLabel, sourceLabel } from "../../types/consistencyBindings";
import "./ShotWorkspace.css";

export interface ScopeConsistencyWorkspaceProps {
  projectId: string;
  scope: ConsistencyScopeRef;
  scopeOptions?: ConsistencyScopeOption[];
  bindingPack?: ConsistencyBindingPack;
  loadBindingPack?: (scope: ConsistencyScopeRef) => Promise<ConsistencyBindingPack>;
  onSaveBindingPack: (input: ConsistencyBindingReplaceInput) => Promise<ConsistencyBindingPack | void>;
  profiles: ConsistencyProfileOption[];
  referenceSets: ConsistencyReferenceSetOption[];
  costumesByCharacter?: Record<string, ConsistencyCostumeOption[]>;
  context?: ConsistencyContextPreview | null;
  stage?: ShotStage;
  loadContext?: (scope: ConsistencyScopeRef, stage: ShotStage) => Promise<ConsistencyContextPreview | null>;
  loading?: boolean;
  saving?: boolean;
  error?: string | null;
  onOpenAssets?: (destination: "profiles" | "referenceSets") => void;
  onScopeChange?: (scope: ConsistencyScopeRef) => void;
}

export type ShotConsistencyPanelProps = ScopeConsistencyWorkspaceProps;

const scopeTitles: Record<ConsistencyScopeType, string> = {
  PROJECT: "项目一致性",
  SERIES: "系列一致性",
  EPISODE: "集一致性",
  SCENE: "场景一致性",
  SHOT: "镜头一致性",
};

export function ScopeConsistencyWorkspace({
  projectId,
  scope,
  scopeOptions = [],
  bindingPack,
  loadBindingPack,
  onSaveBindingPack,
  profiles,
  referenceSets,
  costumesByCharacter = {},
  context,
  stage = "image",
  loadContext,
  loading: externalLoading = false,
  saving: externalSaving = false,
  error: externalError,
  onOpenAssets,
  onScopeChange,
}: ScopeConsistencyWorkspaceProps) {
  const [pack, setPack] = useState<ConsistencyBindingPack>(() => bindingPack ?? emptyBindingPack(scope));
  const isShotScope = scope.scopeType === "SHOT";
  const [resolvedContext, setResolvedContext] = useState<ConsistencyContextPreview | null | undefined>(() => isShotScope ? context : null);
  const [loading, setLoading] = useState(!bindingPack && Boolean(loadBindingPack));
  const [contextLoading, setContextLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [loadError, setLoadError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [dirty, setDirty] = useState(false);

  useEffect(() => {
    let active = true;
    setLoadError(undefined);
    setNotice(undefined);
    setLoading(Boolean(loadBindingPack));
    if (!loadBindingPack && bindingPack) {
      setPack(bindingPack);
      setLoading(false);
      return () => { active = false; };
    }
    if (!loadBindingPack) {
      setPack(bindingPack ?? emptyBindingPack(scope));
      setLoading(false);
      return () => { active = false; };
    }
    void loadBindingPack(scope)
      .then((nextPack) => {
        if (!active) return;
        setPack(nextPack);
        setDirty(false);
      })
      .catch((loadErrorValue: unknown) => {
        if (!active) return;
        setLoadError(errorMessage(loadErrorValue));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => { active = false; };
  }, [bindingPack, loadBindingPack, scope.scopeId, scope.scopeType]);

  useEffect(() => {
    let active = true;
    if (!isShotScope) {
      setResolvedContext(null);
      setContextLoading(false);
      return () => { active = false; };
    }
    if (!loadContext) {
      setResolvedContext(context);
      setContextLoading(false);
      return () => { active = false; };
    }
    setResolvedContext(null);
    setContextLoading(true);
    void loadContext(scope, stage)
      .then((nextContext) => { if (active) setResolvedContext(nextContext); })
      .catch((loadErrorValue: unknown) => { if (active) setLoadError(errorMessage(loadErrorValue)); })
      .finally(() => {
        if (active) setContextLoading(false);
      });
    return () => { active = false; };
  }, [context, isShotScope, loadContext, scope.scopeId, scope.scopeType, stage]);

  const inheritedProfiles = useMemo(
    () => pack.ancestors.flatMap((ancestor) => ancestor.profileBindings.map((binding) => ({ ...binding, inheritanceMode: "INHERITED" as const }))),
    [pack.ancestors],
  );
  const inheritedReferenceSets = useMemo(
    () => pack.ancestors.flatMap((ancestor) => ancestor.referenceSetBindings.map((binding) => ({ ...binding, inheritanceMode: "INHERITED" as const }))),
    [pack.ancestors],
  );
  const busy = externalLoading || loading || contextLoading || externalSaving || saving;
  const error = externalError ?? loadError;

  async function save(input: ConsistencyBindingReplaceInput): Promise<ConsistencyBindingPack | void> {
    setSaving(true);
    setLoadError(undefined);
    setNotice(undefined);
    try {
      const saved = await onSaveBindingPack(input);
      if (saved) setPack(saved);
      if (loadBindingPack) {
        const backendPack = await loadBindingPack(scope);
        setPack(backendPack);
      }
      if (loadContext && isShotScope) setResolvedContext(await loadContext(scope, stage));
      setDirty(false);
      setNotice("一致性配置已保存，并已重新读取后端真值。 ");
      return saved;
    } catch (saveError: unknown) {
      setLoadError(errorMessage(saveError));
      return undefined;
    } finally {
      setSaving(false);
    }
  }

  function requestScopeChange(nextScope: ConsistencyScopeRef) {
    if (nextScope.scopeId === scope.scopeId && nextScope.scopeType === scope.scopeType) return;
    if (dirty && !window.confirm("当前一致性配置尚未保存，确定切换结构范围吗？")) return;
    onScopeChange?.(nextScope);
  }

  return (
    <section className="scope-consistency-workspace" aria-busy={busy} aria-label={`${scopeTitles[scope.scopeType]}工作区`}>
      <header className="consistency-workspace-heading">
        <div>
          <span className="section-label">一致性 / {scopeLabel(scope.scopeType)}</span>
          <h2>{scopeTitles[scope.scopeType]}</h2>
          <p>下级结构默认继承上级配置，也可以在本层替换或移除；保存后以 backend truth 重新加载。</p>
        </div>
        <span className="consistency-scope-name">{scope.scopeName}</span>
      </header>

      {scopeOptions.length > 0 && (
        <nav className="consistency-scope-nav" aria-label="一致性结构范围">
          {scopeOptions.map((option) => <button key={`${option.scopeType}:${option.scopeId}`} type="button" className={isSameScope(option, scope) ? "active" : ""} aria-current={isSameScope(option, scope) ? "page" : undefined} onClick={() => requestScopeChange(option)} disabled={busy}>{scopeLabel(option.scopeType)} · {option.scopeName}</button>)}
        </nav>
      )}

      <section className="consistency-ancestor-path" aria-label="继承路径">
        <div className="consistency-subheading"><strong>结构继承路径</strong><span>{pack.ancestors.length} 个上级范围</span></div>
        <div className="consistency-path-list">
          {pack.ancestors.map((ancestor) => <span key={`${ancestor.scopeType}:${ancestor.scopeId}`} className="consistency-path-item">{scopeLabel(ancestor.scopeType)} · {ancestor.scopeName}</span>)}
          <span className="consistency-path-item consistency-path-current">当前：{scope.scopeName}</span>
        </div>
      </section>

      <ConsistencyBindingEditor
        projectId={projectId}
        scopeType={scope.scopeType}
        scopeId={scope.scopeId}
        directProfileBindings={pack.directProfileBindings}
        directReferenceSetBindings={pack.directReferenceSetBindings}
        inheritedProfileBindings={inheritedProfiles}
        inheritedReferenceSetBindings={inheritedReferenceSets}
        profiles={profiles}
        referenceSets={referenceSets}
        costumesByCharacter={costumesByCharacter}
        loading={busy}
        saving={saving}
        error={error}
        onSave={save}
        onOpenAssets={onOpenAssets}
        onDirtyChange={setDirty}
      />

      {notice && <p className="consistency-notice" role="status">{notice}</p>}
      {scope.scopeType === "SHOT"
        ? <ResolvedContextPreview stage={stage} context={resolvedContext} onCopyHash={copyContextHash} />
        : <p className="consistency-scope-boundary" role="note">最终生成上下文在镜头层计算；当前页面展示本层配置和上级继承关系。</p>}
    </section>
  );
}

function ResolvedContextPreview({ stage, context, onCopyHash }: { stage: ShotStage; context?: ConsistencyContextPreview | null; onCopyHash: (hash: string) => void }) {
  const title = stage === "video" ? "视频解析上下文" : "图片解析上下文";
  return (
    <section className="consistency-resolved-panel" aria-label={title}>
      <div className="consistency-workspace-section-heading">
        <div><span className="section-label">Resolver</span><h3>{title}</h3></div>
        {context?.readinessStatus && <span className="consistency-readiness">Readiness: {context.readinessStatus}</span>}
      </div>
      {!context && <p className="consistency-empty-row">暂未加载最终解析上下文。</p>}
      {context && <>
        {context.partial && <p className="consistency-partial" role="status">解析不完整；请查看下方 diagnostics，Readiness 仍以后端为准。</p>}
        {context.legacy?.usesLegacyShotReferences && <p className="consistency-legacy" role="note">当前使用旧版镜头参考素材</p>}
        {!context.legacy?.usesLegacyShotReferences && context.referenceSets?.length && <p className="consistency-takeover" role="note">一致性参考集已接管本镜头参考输入</p>}
        <div className="consistency-context-meta">
          <div><span>contextHash</span><strong>{context.contextHash ?? "—"}</strong>{context.contextHash && <button type="button" className="quiet-button" onClick={() => onCopyHash(context.contextHash!)}>复制完整 contextHash</button>}</div>
          <div><span>来源</span><strong>{context.sourceTrace?.length ? context.sourceTrace.map(sourceLabel).join(" → ") : "—"}</strong></div>
        </div>
        <p className="consistency-hash-hint">配置变化后会改变；加入生产时冻结。</p>
        <div className="consistency-resolved-columns">
          <ResolvedProfileList profiles={context.profiles ?? []} />
          <ResolvedReferenceList referenceSets={context.referenceSets ?? []} />
        </div>
        <div className="consistency-prompt-grid">
          <div><span>最终解析提示词</span><pre>{context.promptText || context.legacy?.prompt || "—"}</pre></div>
          <div><span>Resolved negative prompt</span><pre>{context.negativePrompt || "—"}</pre></div>
        </div>
        {context.diagnostics.length > 0 && <div className="consistency-diagnostics" aria-label="解析 diagnostics"><strong>Diagnostics</strong>{context.diagnostics.map((diagnostic, index) => <p key={`${diagnostic.code}:${index}`} className={`consistency-diagnostic-${diagnostic.severity.toLowerCase()}`}><span>{diagnostic.severity}</span><strong>{diagnostic.code}</strong>{diagnostic.message}</p>)}</div>}
      </>}
    </section>
  );
}

function ResolvedProfileList({ profiles }: { profiles: NonNullable<ConsistencyContextPreview["profiles"]> }) {
  return <div className="consistency-resolved-list"><div className="consistency-subheading"><strong>最终档案</strong><span>{profiles.length} 项</span></div>{profiles.length ? profiles.map((profile) => <div key={`${profile.role}:${profile.profileId}:${profile.ordinal}`} className="consistency-resolved-row"><span>{roleLabel(profile.role)}</span><strong>{profile.name}</strong><small>{sourceLabel(profile.source)} · #{profile.ordinal + 1}{profile.costumeName ? ` · ${profile.costumeName}` : ""}</small></div>) : <p className="consistency-empty-row">暂无解析档案。</p>}</div>;
}

function ResolvedReferenceList({ referenceSets }: { referenceSets: NonNullable<ConsistencyContextPreview["referenceSets"]> }) {
  return <div className="consistency-resolved-list"><div className="consistency-subheading"><strong>最终参考集与预览</strong><span>{referenceSets.length} 项</span></div>{referenceSets.length ? referenceSets.map((referenceSet) => <div key={`${referenceSet.role}:${referenceSet.referenceSetId}:${referenceSet.ordinal}`} className="consistency-resolved-row"><span>{roleLabel(referenceSet.role)}</span><strong>{referenceSet.name}</strong><small>{sourceLabel(referenceSet.source)} · #{referenceSet.ordinal + 1} · {referenceSet.assetCount ?? referenceSet.previewAssets?.length ?? 0} 个素材{referenceSet.required ? " · 生产必需" : ""}</small>{referenceSet.previewAssets?.length ? <div className="consistency-reference-preview-list">{referenceSet.previewAssets.slice(0, 4).map((asset) => <span key={asset.assetId} title={asset.name ?? asset.assetId}>{asset.name ?? asset.assetId}</span>)}</div> : null}</div>) : <p className="consistency-empty-row">暂无解析参考集。</p>}</div>;
}

function emptyBindingPack(scope: ConsistencyScopeRef): ConsistencyBindingPack {
  return { scope, ancestors: [], directProfileBindings: [], directReferenceSetBindings: [] };
}

function isSameScope(left: ConsistencyScopeRef, right: ConsistencyScopeRef): boolean {
  return left.scopeType === right.scopeType && left.scopeId === right.scopeId;
}

function errorMessage(value: unknown): string {
  if (value instanceof Error) return value.message;
  if (typeof value === "string") return value;
  return "一致性数据加载或保存失败，请稍后重试。";
}

function copyContextHash(hash: string) {
  void navigator.clipboard?.writeText(hash);
}
