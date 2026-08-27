import { useEffect, useMemo, useState } from "react";
import type {
  ConsistencyBindingPack,
  ConsistencyBindingReplaceInput,
  ConsistencyCostumeOption,
  ConsistencyInheritanceMode,
  ConsistencyProfileBindingInput,
  ConsistencyProfileBindingRole,
  ConsistencyProfileOption,
  ConsistencyReferenceSetBindingInput,
  ConsistencyReferenceSetBindingRole,
  ConsistencyReferenceSetOption,
  ConsistencyScopeType,
} from "../../types/consistencyBindings";
import {
  consistencyBindingRoles,
  consistencyReferenceSetRoles,
  normalizeBindingOrdinals,
  profileTypeForRole,
  roleLabel,
} from "../../types/consistencyBindings";
import "./ShotWorkspace.css";

export interface ConsistencyBindingEditorProps {
  projectId: string;
  scopeType: ConsistencyScopeType;
  scopeId: string;
  directProfileBindings: ConsistencyProfileBindingInput[];
  directReferenceSetBindings: ConsistencyReferenceSetBindingInput[];
  inheritedProfileBindings?: ConsistencyProfileBindingInput[];
  inheritedReferenceSetBindings?: ConsistencyReferenceSetBindingInput[];
  profiles: ConsistencyProfileOption[];
  referenceSets: ConsistencyReferenceSetOption[];
  costumesByCharacter?: Record<string, ConsistencyCostumeOption[]>;
  loading?: boolean;
  saving?: boolean;
  error?: string | null;
  onSave: (input: ConsistencyBindingReplaceInput) => Promise<ConsistencyBindingPack | void>;
  onOpenAssets?: (destination: "profiles" | "referenceSets") => void;
  onDirtyChange?: (dirty: boolean) => void;
}
const editableInheritanceModes: ConsistencyInheritanceMode[] = ["EXPLICIT", "REPLACE", "REMOVE"];

export function ConsistencyBindingEditor({
  projectId,
  scopeType,
  scopeId,
  directProfileBindings,
  directReferenceSetBindings,
  inheritedProfileBindings = [],
  inheritedReferenceSetBindings = [],
  profiles,
  referenceSets,
  costumesByCharacter = {},
  loading = false,
  saving = false,
  error,
  onSave,
  onOpenAssets,
  onDirtyChange,
}: ConsistencyBindingEditorProps) {
  const sourceKey = useMemo(
    () => JSON.stringify({ directProfileBindings, directReferenceSetBindings }),
    [directProfileBindings, directReferenceSetBindings],
  );
  const [profileDrafts, setProfileDrafts] = useState<ConsistencyProfileBindingInput[]>(() => directProfileBindings.map(copyProfileBinding));
  const [referenceSetDrafts, setReferenceSetDrafts] = useState<ConsistencyReferenceSetBindingInput[]>(() => directReferenceSetBindings.map(copyReferenceSetBinding));
  const [savedSourceKey, setSavedSourceKey] = useState(sourceKey);
  const [localError, setLocalError] = useState<string>();

  useEffect(() => {
    if (savedSourceKey === sourceKey) return;
    setProfileDrafts(directProfileBindings.map(copyProfileBinding));
    setReferenceSetDrafts(directReferenceSetBindings.map(copyReferenceSetBinding));
    setSavedSourceKey(sourceKey);
    setLocalError(undefined);
  }, [directProfileBindings, directReferenceSetBindings, savedSourceKey, sourceKey]);

  const dirty = useMemo(
    () => JSON.stringify(profileDrafts) !== JSON.stringify(directProfileBindings)
      || JSON.stringify(referenceSetDrafts) !== JSON.stringify(directReferenceSetBindings),
    [directProfileBindings, directReferenceSetBindings, profileDrafts, referenceSetDrafts],
  );

  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);

  const inheritedProfiles = inheritedProfileBindings.filter((binding) => binding.inheritanceMode !== "REMOVE");
  const inheritedReferenceSets = inheritedReferenceSetBindings.filter((binding) => binding.inheritanceMode !== "REMOVE");
  const hasProfiles = profiles.length > 0;
  const hasReferenceSets = referenceSets.length > 0;
  const busy = loading || saving;

  function addProfileBinding() {
    const role: ConsistencyProfileBindingRole = "CHARACTER";
    const option = profiles.find((profile) => profile.profileType === role);
    setLocalError(undefined);
    setProfileDrafts((current) => [...current, {
      role,
      profileType: role,
      profileId: option?.id ?? "",
      costumeVariantId: null,
      ordinal: current.filter((binding) => binding.role === role).length,
      inheritanceMode: "EXPLICIT",
    }]);
  }

  function addReferenceSetBinding() {
    const role: ConsistencyReferenceSetBindingRole = "CHARACTER";
    const option = referenceSets.find((referenceSet) => referenceSet.purpose === role);
    setLocalError(undefined);
    setReferenceSetDrafts((current) => [...current, {
      role,
      referenceSetId: option?.id ?? "",
      ordinal: current.filter((binding) => binding.role === role).length,
      required: false,
      inheritanceMode: "EXPLICIT",
    }]);
  }

  async function save() {
    const profileBindings = normalizeBindingOrdinals(profileDrafts.map(copyProfileBinding));
    const referenceSetBindings = normalizeBindingOrdinals(referenceSetDrafts.map(copyReferenceSetBinding));
    const invalidProfile = profileBindings.find((binding) => !binding.profileId.trim());
    if (invalidProfile) {
      setLocalError(`请选择${roleLabel(invalidProfile.role)}档案后再保存。`);
      return;
    }
    const invalidReferenceSet = referenceSetBindings.find((binding) => !binding.referenceSetId.trim());
    if (invalidReferenceSet) {
      setLocalError(`请选择${roleLabel(invalidReferenceSet.role)}后再保存。`);
      return;
    }
    setLocalError(undefined);
    const result = await onSave({
      projectId,
      scopeType,
      scopeId,
      profileBindings,
      referenceSetBindings,
    });
    if (result) {
      setProfileDrafts(result.directProfileBindings.map(copyProfileBinding));
      setReferenceSetDrafts(result.directReferenceSetBindings.map(copyReferenceSetBinding));
      setSavedSourceKey(JSON.stringify({
        directProfileBindings: result.directProfileBindings,
        directReferenceSetBindings: result.directReferenceSetBindings,
      }));
    }
  }

  return (
    <section className="consistency-binding-editor" aria-busy={busy} aria-label="一致性绑定编辑器">
      <div className="consistency-editor-heading">
        <div>
          <span className="section-label">本层配置</span>
          <h3>绑定档案与参考集</h3>
          <p>新建绑定只使用显式、替换或移除；上级继承项保持只读。</p>
        </div>
        {dirty && <span className="consistency-dirty-badge">未保存</span>}
      </div>

      <div className="consistency-editor-actions">
        <button type="button" className="quiet-button" onClick={addProfileBinding} disabled={busy}>添加档案绑定</button>
        <button type="button" className="quiet-button" onClick={addReferenceSetBinding} disabled={busy}>添加参考集绑定</button>
        <button type="button" className="shot-primary-action" onClick={() => void save()} disabled={busy || !dirty}>
          {saving ? "正在保存…" : "保存一致性配置"}
        </button>
      </div>

      <div className="consistency-binding-columns">
        <div className="consistency-binding-column">
          <div className="consistency-subheading"><strong>档案</strong><span>{profileDrafts.length} 项</span></div>
          {profileDrafts.map((binding, index) => (
            <ProfileBindingRow
              key={binding.id ?? `profile-${index}`}
              binding={binding}
              profiles={profiles}
              costumes={costumesByCharacter[binding.profileId] ?? []}
              disabled={busy}
              onChange={(next) => setProfileDrafts((current) => current.map((item, itemIndex) => itemIndex === index ? next : item))}
              onRemove={() => setProfileDrafts((current) => current.filter((_, itemIndex) => itemIndex !== index))}
            />
          ))}
          {!profileDrafts.length && <p className="consistency-empty-row">还没有本层档案绑定。</p>}
          {!hasProfiles && (
            <div className="consistency-shortcut" role="note">
              <span>还没有可选档案</span>
              {onOpenAssets && <button type="button" className="quiet-button" onClick={() => onOpenAssets("profiles")} disabled={busy}>前往资产库创建</button>}
            </div>
          )}
        </div>

        <div className="consistency-binding-column">
          <div className="consistency-subheading"><strong>参考集</strong><span>{referenceSetDrafts.length} 项</span></div>
          {referenceSetDrafts.map((binding, index) => (
            <ReferenceSetBindingRow
              key={binding.id ?? `reference-set-${index}`}
              binding={binding}
              referenceSets={referenceSets}
              disabled={busy}
              onChange={(next) => setReferenceSetDrafts((current) => current.map((item, itemIndex) => itemIndex === index ? next : item))}
              onRemove={() => setReferenceSetDrafts((current) => current.filter((_, itemIndex) => itemIndex !== index))}
            />
          ))}
          {!referenceSetDrafts.length && <p className="consistency-empty-row">还没有本层参考集绑定。</p>}
          {!hasReferenceSets && (
            <div className="consistency-shortcut" role="note">
              <span>还没有可选参考集</span>
              {onOpenAssets && <button type="button" className="quiet-button" onClick={() => onOpenAssets("referenceSets")} disabled={busy}>前往资产库管理参考集</button>}
            </div>
          )}
        </div>
      </div>

      {(error || localError) && <p className="consistency-error" role="alert">{localError ?? error}</p>}

      <InheritedBindings
        profiles={inheritedProfiles}
        referenceSets={inheritedReferenceSets}
      />
    </section>
  );
}

function ProfileBindingRow({ binding, profiles, costumes, disabled, onChange, onRemove }: {
  binding: ConsistencyProfileBindingInput;
  profiles: ConsistencyProfileOption[];
  costumes: ConsistencyCostumeOption[];
  disabled: boolean;
  onChange: (binding: ConsistencyProfileBindingInput) => void;
  onRemove: () => void;
}) {
  const options = profiles.filter((profile) => profile.profileType === binding.role);
  const canPickCostume = binding.role === "CHARACTER";
  const isHistoricalInherited = binding.inheritanceMode === "INHERITED";
  return (
    <article className="consistency-binding-row" aria-label={`${roleLabel(binding.role)}档案绑定`} data-binding-source={isHistoricalInherited ? "inherited" : "direct"}>
      <div className="consistency-binding-row-heading">
        <strong>{roleLabel(binding.role)}</strong>
        <span>#{binding.ordinal + 1}</span>
        <button type="button" className="quiet-button" onClick={onRemove} disabled={disabled || isHistoricalInherited}>移除本行</button>
      </div>
      <div className="consistency-binding-row-grid">
        <label>
          <span>角色</span>
          <select
            aria-label={`${roleLabel(binding.role)}绑定角色`}
            value={binding.role}
            onChange={(event) => {
              const role = event.target.value as ConsistencyProfileBindingRole;
              const profile = profiles.find((item) => item.profileType === role);
              onChange({ ...binding, role, profileType: profileTypeForRole(role), profileId: profile?.id ?? "", costumeVariantId: null });
            }}
            disabled={disabled || isHistoricalInherited}
          >
            {consistencyBindingRoles.map((role) => <option key={role} value={role}>{roleLabel(role)}</option>)}
          </select>
        </label>
        <label>
          <span>档案</span>
          <select
            aria-label={`${roleLabel(binding.role)}档案`}
            value={binding.profileId}
            onChange={(event) => onChange({ ...binding, profileId: event.target.value, costumeVariantId: null })}
            disabled={disabled || isHistoricalInherited || !options.length}
          >
            <option value="">选择档案</option>
            {options.map((profile) => <option key={profile.id} value={profile.id}>{profile.name} · {profile.profileType}</option>)}
          </select>
        </label>
        {canPickCostume && <label>
          <span>服装</span>
          <select
            aria-label="角色服装"
            value={binding.costumeVariantId ?? ""}
            onChange={(event) => onChange({ ...binding, costumeVariantId: event.target.value || null })}
            disabled={disabled || isHistoricalInherited || !costumes.length}
          >
            <option value="">默认服装</option>
            {costumes.map((costume) => <option key={costume.id} value={costume.id}>{costume.name}</option>)}
          </select>
        </label>}
        <label>
          <span>绑定动作</span>
          <select
            aria-label="绑定动作"
            value={binding.inheritanceMode}
            onChange={(event) => onChange({ ...binding, inheritanceMode: event.target.value as ConsistencyInheritanceMode })}
            disabled={disabled || isHistoricalInherited}
          >
            {editableInheritanceModes.map((mode) => <option key={mode} value={mode}>{inheritanceModeLabel(mode)}</option>)}
            {isHistoricalInherited && <option value="INHERITED">继承</option>}
          </select>
        </label>
      </div>
    </article>
  );
}

function ReferenceSetBindingRow({ binding, referenceSets, disabled, onChange, onRemove }: {
  binding: ConsistencyReferenceSetBindingInput;
  referenceSets: ConsistencyReferenceSetOption[];
  disabled: boolean;
  onChange: (binding: ConsistencyReferenceSetBindingInput) => void;
  onRemove: () => void;
}) {
  const options = referenceSets.filter((referenceSet) => isReferenceSetForRole(referenceSet, binding.role));
  const isHistoricalInherited = binding.inheritanceMode === "INHERITED";
  return (
    <article className="consistency-binding-row" aria-label={`${roleLabel(binding.role)}参考集绑定`} data-binding-source={isHistoricalInherited ? "inherited" : "direct"}>
      <div className="consistency-binding-row-heading">
        <strong>{roleLabel(binding.role)}</strong>
        <span>#{binding.ordinal + 1}</span>
        <button type="button" className="quiet-button" onClick={onRemove} disabled={disabled || isHistoricalInherited}>移除本行</button>
      </div>
      <div className="consistency-binding-row-grid">
        <label>
          <span>角色</span>
          <select aria-label={`${roleLabel(binding.role)}参考集角色`} value={binding.role} onChange={(event) => onChange({ ...binding, role: event.target.value as ConsistencyReferenceSetBindingRole, referenceSetId: "" })} disabled={disabled || isHistoricalInherited}>
            {consistencyReferenceSetRoles.map((role) => <option key={role} value={role}>{roleLabel(role)}</option>)}
          </select>
        </label>
        <label>
          <span>参考集</span>
          <select aria-label={`${roleLabel(binding.role)}参考集`} value={binding.referenceSetId} onChange={(event) => onChange({ ...binding, referenceSetId: event.target.value })} disabled={disabled || isHistoricalInherited || !options.length}>
            <option value="">选择参考集</option>
            {options.map((referenceSet) => <option key={referenceSet.id} value={referenceSet.id}>{referenceSet.name} · {referenceSet.purpose}</option>)}
          </select>
        </label>
        <label className="consistency-required-toggle">
          <input type="checkbox" checked={binding.required} onChange={(event) => onChange({ ...binding, required: event.target.checked })} disabled={disabled || isHistoricalInherited} />
          <span>生产必需</span>
        </label>
        <label>
          <span>绑定动作</span>
          <select aria-label="绑定动作" value={binding.inheritanceMode} onChange={(event) => onChange({ ...binding, inheritanceMode: event.target.value as ConsistencyInheritanceMode })} disabled={disabled || isHistoricalInherited}>
            {editableInheritanceModes.map((mode) => <option key={mode} value={mode}>{inheritanceModeLabel(mode)}</option>)}
            {isHistoricalInherited && <option value="INHERITED">继承</option>}
          </select>
        </label>
      </div>
    </article>
  );
}

function InheritedBindings({ profiles, referenceSets }: { profiles: ConsistencyProfileBindingInput[]; referenceSets: ConsistencyReferenceSetBindingInput[] }) {
  if (!profiles.length && !referenceSets.length) return null;
  return (
    <section className="consistency-inherited-panel" aria-label="上级继承配置">
      <div className="consistency-subheading"><strong>上级继承</strong><span>只读</span></div>
      <div className="consistency-inherited-list">
        {profiles.map((binding, index) => <InheritedRow key={binding.id ?? `profile-${index}`} label={roleLabel(binding.role)} value={binding.profileId} kind="档案" />)}
        {referenceSets.map((binding, index) => <InheritedRow key={binding.id ?? `reference-${index}`} label={roleLabel(binding.role)} value={binding.referenceSetId} kind="参考集" />)}
      </div>
    </section>
  );
}

function InheritedRow({ label, value, kind }: { label: string; value: string; kind: string }) {
  return <div className="consistency-inherited-row" data-binding-source="inherited"><span className="consistency-source-badge">继承</span><strong>{label} · {kind}</strong><code>{value}</code></div>;
}

function copyProfileBinding(binding: ConsistencyProfileBindingInput): ConsistencyProfileBindingInput {
  return { ...binding };
}

function copyReferenceSetBinding(binding: ConsistencyReferenceSetBindingInput): ConsistencyReferenceSetBindingInput {
  return { ...binding };
}

function inheritanceModeLabel(mode: ConsistencyInheritanceMode): string {
  return {
    EXPLICIT: "显式",
    REPLACE: "替换上级配置",
    REMOVE: "移除上级配置",
    INHERITED: "继承",
  }[mode];
}

function isReferenceSetForRole(referenceSet: ConsistencyReferenceSetOption, role: ConsistencyReferenceSetBindingRole): boolean {
  if (role === "SHOT_REFERENCE") return referenceSet.purpose === "SHOT";
  return referenceSet.purpose === role;
}
