import { useEffect, useMemo, useState } from "react";
import {
  deleteRuntimeProfile,
  listRuntimeProfiles,
  saveRuntimeProfile,
} from "../../services/tauriClient";
import type { GenerationValues, RecipeViewModel } from "../../types/generation";
import type { RuntimeParameterProfile } from "../../types/settings";
import { fieldLabel } from "../../i18n/statusLabels";
import {
  applyRuntimeParameterProfile,
  listLegacyRuntimeParameterProfiles,
  migrateLegacyRuntimeProfile,
  removeLegacyRuntimeParameterProfiles,
  sanitizeRuntimeParameterValues,
} from "./pack05";

interface Props {
  recipe: RecipeViewModel;
  values: GenerationValues;
  onApply: (values: GenerationValues) => void;
}

type DraftProfileValues = Record<string, string>;

function emptyDraft(keys: string[]): DraftProfileValues {
  return Object.fromEntries(keys.map((key) => [key, ""]));
}

function profileToDraft(profile: RuntimeParameterProfile | undefined, keys: string[]): DraftProfileValues {
  const draft = emptyDraft(keys);
  if (!profile) return draft;
  for (const key of keys) {
    const value = profile.values[key];
    if (value !== undefined) draft[key] = String(value);
  }
  return draft;
}

function draftToValues(draft: DraftProfileValues): Record<string, number> {
  return sanitizeRuntimeParameterValues(
    Object.fromEntries(
      Object.entries(draft).flatMap(([key, raw]) => {
        if (!raw.trim()) return [];
        const parsed = Number(raw);
        return Number.isSafeInteger(parsed) ? [[key, parsed]] : [];
      }),
    ),
  );
}

function newProfileId(): string {
  return globalThis.crypto?.randomUUID?.() ?? `profile-${Date.now()}`;
}

export function RuntimeParameterProfilePanel({ recipe, values, onApply }: Props) {
  const integerFields = useMemo(
    () => recipe.fields.filter((field): field is Extract<typeof field, { type: "integer" }> => field.type === "integer"),
    [recipe],
  );
  const integerKeys = useMemo(() => integerFields.map((field) => field.key), [integerFields]);
  const recipeKey = `${recipe.workflowVersionId}:${recipe.recipeId}`;
  const [profiles, setProfiles] = useState<RuntimeParameterProfile[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [profileName, setProfileName] = useState("");
  const [draft, setDraft] = useState<DraftProfileValues>(() => emptyDraft(integerKeys));
  const [notice, setNotice] = useState<string>();
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setNotice(undefined);
    void (async () => {
      try {
        let allProfiles = await listRuntimeProfiles();
        if (!allProfiles.length) {
          const legacyProfiles = listLegacyRuntimeParameterProfiles();
          const migrated = legacyProfiles.map((profile) => migrateLegacyRuntimeProfile(recipe, profile));
          const unresolved = migrated.flatMap((result) => result.unresolvedKeys);
          if (legacyProfiles.length && !unresolved.length) {
            try {
              const saved: RuntimeParameterProfile[] = [];
              for (const result of migrated) {
                if (result.profile) saved.push(await saveRuntimeProfile(result.profile));
              }
              removeLegacyRuntimeParameterProfiles();
              allProfiles = saved;
              if (active) setNotice("旧版参数档案已迁移到应用设置文件。");
            } catch (error: unknown) {
              if (active) setNotice(`旧版参数档案迁移未完成，旧数据仍保留：${toErrorMessage(error)}`);
            }
          } else if (legacyProfiles.length && active) {
            setNotice(`旧版参数档案包含无法映射的字段（${[...new Set(unresolved)].join("、")}），旧数据仍保留，请手动重建。`);
          }
        }
        if (!active) return;
        const matching = allProfiles.filter((profile) => `${profile.workflowVersionId}:${profile.recipeId}` === recipeKey);
        setProfiles(matching);
        setSelectedId(matching[0]?.id ?? "");
        setProfileName(matching[0]?.name ?? "");
        setDraft(profileToDraft(matching[0], integerKeys));
      } catch (error: unknown) {
        if (active) setNotice(`参数档案读取失败：${toErrorMessage(error)}`);
      } finally {
        if (active) setLoading(false);
      }
    })();
    return () => {
      active = false;
    };
  }, [integerKeys, recipe, recipeKey]);

  const selectedProfile = useMemo(
    () => profiles.find((profile) => profile.id === selectedId),
    [profiles, selectedId],
  );

  function selectProfile(id: string) {
    const profile = profiles.find((item) => item.id === id);
    setSelectedId(id);
    setProfileName(profile?.name ?? "");
    setDraft(profileToDraft(profile, integerKeys));
    setNotice(undefined);
  }

  async function saveProfile() {
    const name = profileName.trim();
    if (!name) {
      setNotice("请输入参数档案名称。");
      return;
    }
    setSaving(true);
    setNotice(undefined);
    try {
      const profile = await saveRuntimeProfile({
        id: selectedId || newProfileId(),
        workflowVersionId: recipe.workflowVersionId,
        recipeId: recipe.recipeId,
        name,
        values: draftToValues(draft),
        updatedAt: new Date().toISOString(),
      });
      setProfiles((current) => [profile, ...current.filter((item) => item.id !== profile.id)]);
      setSelectedId(profile.id);
      setProfileName(profile.name);
      setDraft(profileToDraft(profile, integerKeys));
      setNotice("参数档案已保存到应用设置；素材输入不会写入档案。");
    } catch (error: unknown) {
      setNotice(`参数档案保存失败：${toErrorMessage(error)}`);
    } finally {
      setSaving(false);
    }
  }

  function applyProfile() {
    if (!selectedProfile) {
      setNotice("请先选择参数档案。");
      return;
    }
    const profileWithDraftValues = { ...selectedProfile, values: draftToValues(draft) };
    const result = applyRuntimeParameterProfile(recipe, values, profileWithDraftValues);
    onApply(result.values);
    setNotice(result.appliedFields.length
      ? `已应用 ${result.appliedFields.length} 个整数字段${result.ignoredParameters.length ? `；忽略不存在的字段：${result.ignoredParameters.join("、")}` : ""}。`
      : "当前工作流没有可绑定的整数字段，档案仍已保留。",
    );
  }

  async function removeProfile() {
    if (!selectedProfile) return;
    setSaving(true);
    setNotice(undefined);
    try {
      await deleteRuntimeProfile(selectedProfile.id);
      const next = profiles.filter((profile) => profile.id !== selectedProfile.id);
      setProfiles(next);
      setSelectedId(next[0]?.id ?? "");
      setProfileName(next[0]?.name ?? "");
      setDraft(profileToDraft(next[0], integerKeys));
      setNotice("参数档案已删除。");
    } catch (error: unknown) {
      setNotice(`参数档案删除失败：${toErrorMessage(error)}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <details className="runtime-profile-panel">
      <summary>运行时参数档案</summary>
      <div className="runtime-profile-content">
        <p>保存当前配方的整数输入；应用时按字段键直接绑定，素材、并发设置和名称猜测都不会写入档案。</p>
        <div className="runtime-profile-toolbar">
          <select aria-label="运行时参数档案" value={selectedId} onChange={(event) => selectProfile(event.target.value)} disabled={loading || saving}>
            <option value="">新建参数档案</option>
            {profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
          </select>
          <button type="button" onClick={applyProfile} disabled={!selectedProfile || loading || saving}>应用</button>
          <button type="button" className="quiet-button" onClick={() => void saveProfile()} disabled={loading || saving}>保存</button>
          <button type="button" className="quiet-button" onClick={() => void removeProfile()} disabled={!selectedProfile || loading || saving}>删除</button>
        </div>
        <label className="runtime-profile-name">
          <span>档案名称</span>
          <input value={profileName} maxLength={80} onChange={(event) => setProfileName(event.target.value)} placeholder="例如：低显存预览" />
        </label>
        <div className="runtime-profile-grid">
          {integerFields.map((field) => (
            <label key={field.key}>
              <span>{fieldLabel(field.key, field.label)} <small>{field.min !== undefined && field.max !== undefined ? `（${field.min}–${field.max}）` : ""}</small></span>
              <input inputMode="numeric" value={draft[field.key] ?? ""} onChange={(event) => setDraft((current) => ({ ...current, [field.key]: event.target.value }))} placeholder="未设置" />
            </label>
          ))}
        </div>
        {!integerFields.length && <small>当前配方没有可保存的整数字段。</small>}
        {notice && <p className="runtime-profile-notice" role="status">{notice}</p>}
      </div>
    </details>
  );
}

function toErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error && typeof error.message === "string") return error.message;
  return "请稍后重试";
}
