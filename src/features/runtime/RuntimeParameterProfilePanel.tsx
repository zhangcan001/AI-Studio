import { useEffect, useMemo, useState } from "react";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import {
  applyRuntimeParameterProfile,
  deleteRuntimeParameterProfile,
  listRuntimeParameterProfiles,
  runtimeParameterLabel,
  runtimeProfileKey,
  sanitizeRuntimeParameterValues,
  saveRuntimeParameterProfile,
  type RuntimeParameterKey,
  type RuntimeParameterProfile,
} from "./pack05";

interface Props {
  recipe: RecipeViewModel;
  values: GenerationValues;
  onApply: (values: GenerationValues) => void;
}

const editableKeys: RuntimeParameterKey[] = ["steps", "width", "height", "durationSeconds", "concurrency"];

type DraftProfileValues = Record<RuntimeParameterKey, string>;

function emptyDraft(): DraftProfileValues {
  return { steps: "", width: "", height: "", durationSeconds: "", concurrency: "" };
}

function profileToDraft(profile?: RuntimeParameterProfile): DraftProfileValues {
  const draft = emptyDraft();
  if (!profile) return draft;
  for (const key of editableKeys) {
    const value = profile.values[key];
    if (value !== undefined) draft[key] = String(value);
  }
  return draft;
}

function draftToValues(draft: DraftProfileValues) {
  const values: Partial<Record<RuntimeParameterKey, number>> = {};
  for (const key of editableKeys) {
    if (!draft[key].trim()) continue;
    const parsed = Number(draft[key]);
    if (Number.isFinite(parsed)) values[key] = parsed;
  }
  return sanitizeRuntimeParameterValues(values);
}

function newProfileId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `profile-${Date.now()}`;
}

export function RuntimeParameterProfilePanel({ recipe, values, onApply }: Props) {
  const [profiles, setProfiles] = useState<RuntimeParameterProfile[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [profileName, setProfileName] = useState("");
  const [draft, setDraft] = useState<DraftProfileValues>(emptyDraft);
  const [notice, setNotice] = useState<string>();
  const recipeKey = runtimeProfileKey(recipe);

  useEffect(() => {
    const matching = listRuntimeParameterProfiles().filter((profile) => runtimeProfileKey(profile) === recipeKey);
    setProfiles(matching);
    setSelectedId(matching[0]?.id ?? "");
    setProfileName(matching[0]?.name ?? "");
    setDraft(profileToDraft(matching[0]));
    setNotice(undefined);
  }, [recipeKey]);

  const selectedProfile = useMemo(() => profiles.find((profile) => profile.id === selectedId), [profiles, selectedId]);

  function selectProfile(id: string) {
    const profile = profiles.find((item) => item.id === id);
    setSelectedId(id);
    setProfileName(profile?.name ?? "");
    setDraft(profileToDraft(profile));
    setNotice(undefined);
  }

  function saveProfile() {
    const name = profileName.trim();
    if (!name) {
      setNotice("请输入参数档案名称。");
      return;
    }
    const profile = saveRuntimeParameterProfile({
      id: selectedId || newProfileId(),
      workflowVersionId: recipe.workflowVersionId,
      recipeId: recipe.recipeId,
      name,
      values: draftToValues(draft),
      updatedAt: new Date().toISOString(),
    });
    setProfiles((current) => [profile, ...current.filter((item) => item.id !== profile.id)]);
    setSelectedId(profile.id);
    setNotice("参数档案已保存；素材输入不会写入档案。");
  }

  function applyProfile() {
    if (!selectedProfile) {
      setNotice("请先选择参数档案。");
      return;
    }
    const profileWithDraftValues = { ...selectedProfile, values: draftToValues(draft) };
    const result = applyRuntimeParameterProfile(recipe, values, profileWithDraftValues);
    onApply(result.values);
    const ignored = result.ignoredParameters.map(runtimeParameterLabel).join("、");
    setNotice(result.appliedFields.length
      ? `已应用 ${result.appliedFields.length} 个输入参数${ignored ? `；未找到${ignored}` : ""}。`
      : "当前工作流没有可绑定的整数字段，档案仍已保留。",
    );
  }

  function removeProfile() {
    if (!selectedProfile) return;
    deleteRuntimeParameterProfile(selectedProfile.id);
    const next = profiles.filter((profile) => profile.id !== selectedProfile.id);
    setProfiles(next);
    setSelectedId(next[0]?.id ?? "");
    setProfileName(next[0]?.name ?? "");
    setDraft(profileToDraft(next[0]));
    setNotice("参数档案已删除。");
  }

  return (
    <details className="runtime-profile-panel">
      <summary>运行时参数档案</summary>
      <div className="runtime-profile-content">
        <p>保存当前工作流的通用尺寸、步数与时长配置；应用时只写入已映射的整数字段。</p>
        <div className="runtime-profile-toolbar">
          <select aria-label="运行时参数档案" value={selectedId} onChange={(event) => selectProfile(event.target.value)}>
            <option value="">新建参数档案</option>
            {profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
          </select>
          <button type="button" onClick={applyProfile} disabled={!selectedProfile}>应用</button>
          <button type="button" className="quiet-button" onClick={saveProfile}>保存</button>
          <button type="button" className="quiet-button" onClick={removeProfile} disabled={!selectedProfile}>删除</button>
        </div>
        <label className="runtime-profile-name"><span>档案名称</span><input value={profileName} maxLength={80} onChange={(event) => setProfileName(event.target.value)} placeholder="例如：低显存预览" /></label>
        <div className="runtime-profile-grid">
          {editableKeys.map((key) => (
            <label key={key}><span>{runtimeParameterLabel(key)}</span><input inputMode="numeric" value={draft[key]} onChange={(event) => setDraft((current) => ({ ...current, [key]: event.target.value }))} placeholder="未设置" /></label>
          ))}
        </div>
        <small>并发上限只作为队列策略记录；当前生产队列仍按严格顺序执行。</small>
        {notice && <p className="runtime-profile-notice" role="status">{notice}</p>}
      </div>
    </details>
  );
}
