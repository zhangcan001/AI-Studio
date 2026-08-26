import { useMemo, useState } from "react";
import { toUserMessage } from "../../i18n/errorMessages";
import type {
  ConsistencyProfileDraft,
  ConsistencyProfileView,
  CostumeVariantRequest,
  CostumeVariantUpdateRequest,
  CostumeVariantView,
  ProfileType,
  ReferenceSetPurpose,
  ReferenceSetSummary,
} from "../../types/consistency";

const profileLabels: Record<ProfileType, string> = {
  CHARACTER: "角色",
  SCENE: "场景",
  PROP: "道具",
  STYLE: "风格",
};

const purposeLabels: Record<ReferenceSetPurpose, string> = {
  CHARACTER: "角色",
  COSTUME: "服装",
  SCENE: "场景",
  PROP: "道具",
  STYLE: "风格",
  SHOT: "镜头",
};

type TextDraftField = Exclude<keyof ConsistencyProfileDraft, "profileType">;

interface Props {
  profileType: ProfileType;
  profile?: ConsistencyProfileView;
  costumes?: CostumeVariantView[];
  referenceSets?: ReferenceSetSummary[];
  styleProfiles?: ConsistencyProfileView[];
  onSave: (draft: ConsistencyProfileDraft) => Promise<void>;
  onCancel?: () => void;
  onDelete?: () => Promise<void>;
  onCostumeCreate?: (request: CostumeVariantRequest) => Promise<void>;
  onCostumeUpdate?: (request: CostumeVariantUpdateRequest) => Promise<void>;
  onCostumeDelete?: (variant: CostumeVariantView) => Promise<void>;
  onDirtyChange?: (dirty: boolean) => void;
  busy?: boolean;
  deleteBlocked?: boolean;
  error?: string;
}

function profileDraft(profileType: ProfileType, profile?: ConsistencyProfileView): ConsistencyProfileDraft {
  return {
    profileType,
    name: profile?.name ?? "",
    description: profile?.description ?? "",
    canonicalPrompt: profile?.canonicalPrompt ?? "",
    negativePrompt: profile?.negativePrompt ?? "",
    environmentPrompt: profile?.environmentPrompt ?? "",
    lightingPrompt: profile?.lightingPrompt ?? "",
    materialPrompt: profile?.materialPrompt ?? "",
    scalePrompt: profile?.scalePrompt ?? "",
    stylePrompt: profile?.stylePrompt ?? "",
    colorPrompt: profile?.colorPrompt ?? "",
    linePrompt: profile?.linePrompt ?? "",
    outputNotes: profile?.outputNotes ?? "",
    defaultReferenceSetId: profile?.defaultReferenceSetId ?? "",
    defaultStyleProfileId: profile?.defaultStyleProfileId ?? "",
    metadataJson: profile?.metadataJson ?? "{}",
  };
}

function emptyCostume(characterProfileId: string, variants: CostumeVariantView[]): CostumeForm {
  return {
    id: undefined,
    characterProfileId,
    name: "",
    promptFragment: "",
    referenceSetId: "",
    isDefault: variants.length === 0,
    ordinal: variants.length ? Math.max(...variants.map((variant) => variant.ordinal)) + 1 : 0,
  };
}

interface CostumeForm {
  id?: string;
  characterProfileId: string;
  name: string;
  promptFragment: string;
  referenceSetId: string;
  isDefault: boolean;
  ordinal: number;
}

function referenceSetName(referenceSets: ReferenceSetSummary[], id?: string | null): string {
  if (!id) return "未绑定";
  return referenceSets.find((set) => set.id === id)?.name ?? id;
}

export function ConsistencyProfileEditor({
  profileType,
  profile,
  costumes = [],
  referenceSets = [],
  styleProfiles = [],
  onSave,
  onCancel,
  onDelete,
  onCostumeCreate,
  onCostumeUpdate,
  onCostumeDelete,
  onDirtyChange,
  busy = false,
  deleteBlocked = false,
  error,
}: Props) {
  const [draft, setDraft] = useState<ConsistencyProfileDraft>(() => profileDraft(profileType, profile));
  const [dirty, setDirty] = useState(false);
  const [formError, setFormError] = useState<string>();
  const [saving, setSaving] = useState(false);
  const [costumeForm, setCostumeForm] = useState<CostumeForm>();
  const [costumeError, setCostumeError] = useState<string>();
  const [costumeBusy, setCostumeBusy] = useState(false);

  const savingNow = busy || saving;
  const matchingReferenceSets = useMemo(
    () => referenceSets.filter((set) => set.purpose === profileType || set.purpose === "COSTUME"),
    [profileType, referenceSets],
  );
  const costumeReferenceSets = useMemo(
    () => referenceSets.filter((set) => set.purpose === "COSTUME"),
    [referenceSets],
  );
  const availableStyleProfiles = useMemo(
    () => styleProfiles.filter((item) => item.profileType === "STYLE" && item.id !== profile?.id),
    [profile?.id, styleProfiles],
  );

  function markDirty() {
    setDirty((current) => {
      if (!current) onDirtyChange?.(true);
      return true;
    });
  }

  function setText(field: TextDraftField, value: string) {
    setDraft((current) => ({ ...current, [field]: value }));
    markDirty();
    setFormError(undefined);
  }

  async function save() {
    if (!draft.name.trim()) {
      setFormError("请输入档案名称。");
      return;
    }
    setFormError(undefined);
    setSaving(true);
    try {
      await onSave({ ...draft, name: draft.name.trim() });
      setDirty(false);
      onDirtyChange?.(false);
    } catch (value: unknown) {
      setFormError(toUserMessage(value));
    } finally {
      setSaving(false);
    }
  }

  function openCostumeForm() {
    if (!profile) {
      setCostumeError("请先保存角色档案，再添加服装变体。");
      return;
    }
    setCostumeError(undefined);
    setCostumeForm(emptyCostume(profile.id, costumes));
  }

  function editCostume(variant: CostumeVariantView) {
    setCostumeError(undefined);
    setCostumeForm({
      id: variant.id,
      characterProfileId: variant.characterProfileId,
      name: variant.name,
      promptFragment: variant.promptFragment,
      referenceSetId: variant.referenceSetId ?? "",
      isDefault: variant.isDefault,
      ordinal: variant.ordinal,
    });
  }

  async function saveCostume() {
    if (!costumeForm) return;
    if (!costumeForm.name.trim()) {
      setCostumeError("请输入服装变体名称。");
      return;
    }
    if (!profile) {
      setCostumeError("请先保存角色档案，再添加服装变体。");
      return;
    }
    const common = {
      projectId: profile.projectId,
      name: costumeForm.name.trim(),
      promptFragment: costumeForm.promptFragment,
      referenceSetId: costumeForm.referenceSetId || null,
      isDefault: costumeForm.isDefault,
      ordinal: Math.max(0, Math.floor(costumeForm.ordinal)),
    };
    setCostumeBusy(true);
    setCostumeError(undefined);
    try {
      if (costumeForm.id) {
        if (!onCostumeUpdate) throw new Error("服装变体更新暂不可用。");
        await onCostumeUpdate({ ...common, costumeVariantId: costumeForm.id });
      } else {
        if (!onCostumeCreate) throw new Error("服装变体创建暂不可用。");
        await onCostumeCreate({ ...common, characterProfileId: profile.id });
      }
      setCostumeForm(undefined);
    } catch (value: unknown) {
      setCostumeError(toUserMessage(value));
    } finally {
      setCostumeBusy(false);
    }
  }

  async function removeCostume(variant: CostumeVariantView) {
    if (!onCostumeDelete || !window.confirm(`确定删除服装变体“${variant.name}”吗？`)) return;
    setCostumeBusy(true);
    setCostumeError(undefined);
    try {
      await onCostumeDelete(variant);
      if (costumeForm?.id === variant.id) setCostumeForm(undefined);
    } catch (value: unknown) {
      setCostumeError(toUserMessage(value));
    } finally {
      setCostumeBusy(false);
    }
  }

  function updateCostume(field: keyof CostumeForm, value: string | number | boolean) {
    setCostumeForm((current) => current ? { ...current, [field]: value } : current);
    setCostumeError(undefined);
  }

  function styleReferenceOptions(): ReferenceSetSummary[] {
    return matchingReferenceSets.filter((set) => set.purpose === profileType);
  }

  function relationSelectors(includeReferenceSet: boolean) {
    return (
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(180px, 1fr))", gap: 10 }}>
        {includeReferenceSet && (
          <label className="field-control">
            <span>默认参考集</span>
            <select value={draft.defaultReferenceSetId} onChange={(event) => setText("defaultReferenceSetId", event.target.value)} disabled={savingNow}>
              <option value="">不设置</option>
              {styleReferenceOptions().map((set) => <option key={set.id} value={set.id}>{set.name}</option>)}
            </select>
          </label>
        )}
        <label className="field-control">
          <span>默认风格 Profile</span>
          <select value={draft.defaultStyleProfileId} onChange={(event) => setText("defaultStyleProfileId", event.target.value)} disabled={savingNow}>
            <option value="">不设置</option>
            {availableStyleProfiles.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
          </select>
        </label>
      </div>
    );
  }

  function renderTextField(field: TextDraftField, label: string, placeholder?: string, rows = 3) {
    return (
      <label className="field-control">
        <span>{label}</span>
        <textarea value={draft[field]} onChange={(event) => setText(field, event.target.value)} placeholder={placeholder} rows={rows} disabled={savingNow} />
      </label>
    );
  }

  return (
    <section className="consistency-profile-editor" aria-label={profile ? `编辑${profileLabels[profileType]}档案` : `新建${profileLabels[profileType]}档案`} style={{ display: "grid", gap: 14, minWidth: 0 }}>
      <div className="section-heading" style={{ alignItems: "flex-start", marginBottom: 0 }}>
        <div>
          <span className="section-label">{profile ? "档案编辑" : "新建档案"}</span>
          <h3>{profile ? `编辑${profileLabels[profileType]}档案` : `新建${profileLabels[profileType]}档案`}</h3>
          <p className="section-description">保存后会成为当前项目的语义资产，可在后续镜头上下文中复用。</p>
        </div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap", justifyContent: "flex-end" }}>
          {onCancel && <button type="button" className="quiet-button" onClick={onCancel} disabled={savingNow}>取消</button>}
          {profile && onDelete && <button type="button" className="danger-button" onClick={() => void onDelete()} disabled={savingNow || deleteBlocked}>删除档案</button>}
        </div>
      </div>

      {dirty && <p className="disabled-note" role="status">有未保存的修改，切换档案前会再次确认。</p>}
      {(error || formError) && <p className="error-message" role="alert">{formError ?? error}</p>}

      <div style={{ display: "grid", gap: 10 }}>
        <label className="field-control">
          <span>名称</span>
          <input value={draft.name} maxLength={120} onChange={(event) => setText("name", event.target.value)} placeholder={`例如：${profileLabels[profileType]} 01`} disabled={savingNow} />
        </label>
        {profileType !== "STYLE" && renderTextField("description", "描述", "用一句话说明这个档案的稳定语义。", 2)}

        {profileType === "CHARACTER" && (
          <>
            {renderTextField("canonicalPrompt", "角色提示词", "角色身份、外观和不可变化的特征。", 4)}
            {renderTextField("negativePrompt", "负面提示词", "不希望出现的角色特征，可选。", 3)}
            {relationSelectors(true)}
          </>
        )}
        {profileType === "SCENE" && (
          <>
            {renderTextField("environmentPrompt", "环境提示词", "空间、时间和环境身份。", 4)}
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 10 }}>
              {renderTextField("lightingPrompt", "灯光提示词", "可选。", 3)}
              {renderTextField("negativePrompt", "负面提示词", "可选。", 3)}
            </div>
            {relationSelectors(true)}
          </>
        )}
        {profileType === "PROP" && (
          <>
            {renderTextField("canonicalPrompt", "道具提示词", "道具的身份、外观和用途。", 4)}
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 10 }}>
              {renderTextField("materialPrompt", "材质提示词", "可选。", 3)}
              {renderTextField("scalePrompt", "尺度提示词", "可选。", 3)}
            </div>
            <label className="field-control">
              <span>默认参考集</span>
              <select value={draft.defaultReferenceSetId} onChange={(event) => setText("defaultReferenceSetId", event.target.value)} disabled={savingNow}>
                <option value="">不设置</option>
                {styleReferenceOptions().map((set) => <option key={set.id} value={set.id}>{set.name}</option>)}
              </select>
            </label>
          </>
        )}
        {profileType === "STYLE" && (
          <>
            {renderTextField("stylePrompt", "风格提示词", "画风、媒介和整体视觉语言。", 4)}
            <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fit, minmax(220px, 1fr))", gap: 10 }}>
              {renderTextField("colorPrompt", "色彩提示词", "可选。", 3)}
              {renderTextField("linePrompt", "线条提示词", "可选。", 3)}
              {renderTextField("negativePrompt", "负面提示词", "可选。", 3)}
            </div>
            {renderTextField("outputNotes", "输出说明", "给生成参数或后续流程的补充说明。", 3)}
          </>
        )}

        {profileType === "CHARACTER" && (
          <details className="consistency-profile-advanced">
            <summary>高级字段</summary>
            <label className="field-control">
              <span>metadataJson</span>
              <textarea value={draft.metadataJson} onChange={(event) => setText("metadataJson", event.target.value)} rows={3} placeholder="普通用户通常不需要填写。" disabled={savingNow} />
            </label>
          </details>
        )}
      </div>

      {profileType === "CHARACTER" && (
        <section className="consistency-costume-section" aria-label="服装变体" style={{ display: "grid", gap: 10, paddingTop: 8, borderTop: "1px solid var(--studio-border, rgba(255,255,255,.08))" }}>
          <div className="section-heading" style={{ marginBottom: 0 }}>
            <div>
              <strong>服装变体</strong>
              <p className="section-description">角色身份保持不变，服装与造型通过 CostumeVariant 管理。</p>
            </div>
            <button type="button" onClick={openCostumeForm} disabled={savingNow || costumeBusy || !profile}>新增服装变体</button>
          </div>
          {!profile && <p className="empty-state">保存角色档案后即可添加服装变体。</p>}
          {profile && !costumes.length && <p className="empty-state">当前角色还没有服装变体。</p>}
          {costumes.map((variant) => (
            <article key={variant.id} className="consistency-costume-row" style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr) auto", gap: 8, alignItems: "center", padding: 10, border: "1px solid var(--studio-border, rgba(255,255,255,.08))", borderRadius: 8 }}>
              <div style={{ minWidth: 0 }}>
                <strong>{variant.name}</strong>
                <small style={{ display: "block", color: "var(--studio-text-secondary, #9ca3af)" }}>
                  {variant.promptFragment || "无提示词片段"} · {referenceSetName(costumeReferenceSets, variant.referenceSetId)} · 顺序 {variant.ordinal}
                </small>
              </div>
              <div style={{ display: "flex", gap: 6, flexWrap: "wrap", justifyContent: "flex-end" }}>
                {variant.isDefault && <span className="status-pill">默认</span>}
                <button type="button" className="quiet-button" onClick={() => editCostume(variant)} disabled={savingNow || costumeBusy}>编辑</button>
                <button type="button" className="danger-button" onClick={() => void removeCostume(variant)} disabled={savingNow || costumeBusy}>删除</button>
              </div>
            </article>
          ))}
          {costumeForm && (
            <div className="consistency-costume-form" style={{ display: "grid", gap: 9, padding: 12, border: "1px solid var(--studio-border-strong, rgba(255,255,255,.12))", borderRadius: 8 }}>
              <strong>{costumeForm.id ? "编辑服装变体" : "新增服装变体"}</strong>
              <label className="field-control"><span>名称</span><input value={costumeForm.name} onChange={(event) => updateCostume("name", event.target.value)} disabled={costumeBusy || savingNow} /></label>
              <label className="field-control"><span>提示词片段</span><textarea value={costumeForm.promptFragment} onChange={(event) => updateCostume("promptFragment", event.target.value)} rows={3} placeholder="例如：穿着深蓝色长袍" disabled={costumeBusy || savingNow} /></label>
              <div style={{ display: "grid", gridTemplateColumns: "minmax(0, 1fr) 110px", gap: 9 }}>
                <label className="field-control"><span>服装参考集</span><select value={costumeForm.referenceSetId} onChange={(event) => updateCostume("referenceSetId", event.target.value)} disabled={costumeBusy || savingNow}><option value="">不设置</option>{costumeReferenceSets.map((set) => <option key={set.id} value={set.id}>{set.name}</option>)}</select></label>
                <label className="field-control"><span>顺序</span><input type="number" min={0} step={1} value={costumeForm.ordinal} onChange={(event) => updateCostume("ordinal", Number(event.target.value))} disabled={costumeBusy || savingNow} /></label>
              </div>
              <label className="check-control"><input type="checkbox" checked={costumeForm.isDefault} onChange={(event) => updateCostume("isDefault", event.target.checked)} disabled={costumeBusy || savingNow} /><span>设为默认服装（会取消其他默认项）</span></label>
              {costumeError && <p className="error-message" role="alert">{costumeError}</p>}
              <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", flexWrap: "wrap" }}>
                <button type="button" onClick={() => void saveCostume()} disabled={costumeBusy || savingNow || !costumeForm.name.trim()}>{costumeBusy ? "正在保存…" : "保存服装变体"}</button>
                <button type="button" className="quiet-button" onClick={() => setCostumeForm(undefined)} disabled={costumeBusy || savingNow}>取消</button>
              </div>
            </div>
          )}
        </section>
      )}

      <div style={{ display: "flex", gap: 8, justifyContent: "flex-end", flexWrap: "wrap" }}>
        <button type="button" className="primary-action" onClick={() => void save()} disabled={savingNow || !draft.name.trim()}>{saving ? "正在保存…" : profile ? "保存档案" : "创建档案"}</button>
        {onCancel && <button type="button" className="quiet-button" onClick={onCancel} disabled={savingNow}>取消</button>}
      </div>
    </section>
  );
}

export { profileDraft, purposeLabels };
