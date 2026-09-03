import { useEffect, useMemo, useState } from "react";
import {
  clearProjectWorkflowOverrides,
  clearSelectedRecipeRef,
  filterImageRecipes,
  filterVideoRecipes,
  readProjectWorkflowOverrides,
  readSelectedRecipeRef,
  recipesForVideoMode,
  type H3CompatibleMode,
  type SelectedRecipeRef,
} from "../runtime/workflowCapabilities";
import { getProjectWorkflowConfig, replaceProjectWorkflowConfig } from "../../services/tauriClient";
import type { RecipeViewModel } from "../../types/generation";
import type {
  ProjectWorkflowBindingInput,
  ProjectWorkflowBindingView,
  ProjectWorkflowConfigView,
  ProjectWorkflowMode,
} from "../../types/projectWorkflow";
import { toUserMessage } from "../../i18n/errorMessages";
import { workflowDisplayName } from "../../i18n/statusLabels";

interface Props {
  projectId: string;
  catalog: RecipeViewModel[];
  onConfigChanged?: (config: ProjectWorkflowConfigView) => void;
}

type Draft = {
  imageDefault?: SelectedRecipeRef;
  videoDefault?: SelectedRecipeRef;
  videoModeOverrides: Partial<Record<H3CompatibleMode, SelectedRecipeRef>>;
};

const VIDEO_MODE_LABELS: Record<H3CompatibleMode, string> = {
  FL2VA_TEXT_TO_VIDEO: "文生视频",
  FL2VA_IMAGE_TO_VIDEO: "图生视频",
  FL2VA_FIRST_LAST: "首尾帧视频",
  REF2VA_IMAGE: "参考图视频",
  REF2VA_AUDIO: "参考音频视频",
  REF2VA_IMAGE_AUDIO: "参考图 + 音频",
  REF2VA_VIDEO_IMAGE: "参考视频 + 参考图",
};
const VIDEO_MODES = Object.keys(VIDEO_MODE_LABELS) as H3CompatibleMode[];
const UNSET = "__unset__";

function refKey(ref: SelectedRecipeRef | undefined): string {
  return ref ? `${ref.workflowVersionId}:${ref.recipeId}` : UNSET;
}

function parseRef(value: string): SelectedRecipeRef | undefined {
  if (value === UNSET) return undefined;
  const separator = value.indexOf(":");
  if (separator <= 0 || separator === value.length - 1) return undefined;
  return { workflowVersionId: value.slice(0, separator), recipeId: value.slice(separator + 1) };
}

function viewRef(binding: ProjectWorkflowBindingView | null | undefined): SelectedRecipeRef | undefined {
  return binding
    ? { workflowVersionId: binding.workflowVersionId, recipeId: binding.recipeId }
    : undefined;
}

function draftFromConfig(config: ProjectWorkflowConfigView): Draft {
  return {
    imageDefault: viewRef(config.imageDefault),
    videoDefault: viewRef(config.videoDefault),
    videoModeOverrides: Object.fromEntries(
      config.videoModeOverrides.map((binding) => [binding.mode, viewRef(binding)]),
    ) as Partial<Record<H3CompatibleMode, SelectedRecipeRef>>,
  };
}

function configIsEmpty(config: ProjectWorkflowConfigView): boolean {
  return !config.imageDefault && !config.videoDefault && config.videoModeOverrides.length === 0;
}

function bindingInput(
  stage: "IMAGE" | "VIDEO",
  mode: ProjectWorkflowMode,
  ref: SelectedRecipeRef | undefined,
): ProjectWorkflowBindingInput | undefined {
  return ref ? { stage, mode, ...ref } : undefined;
}

function draftBindings(draft: Draft): ProjectWorkflowBindingInput[] {
  const bindings = [
    bindingInput("IMAGE", "DEFAULT", draft.imageDefault),
    bindingInput("VIDEO", "DEFAULT", draft.videoDefault),
    ...VIDEO_MODES.map((mode) => bindingInput("VIDEO", mode, draft.videoModeOverrides[mode])),
  ];
  return bindings.filter((binding): binding is ProjectWorkflowBindingInput => Boolean(binding));
}

function recipeLabel(recipe: RecipeViewModel): string {
  return `${workflowDisplayName(recipe.workflowId, recipe.name)} · ${recipe.workflowVersionId} · ${recipe.recipeId}`;
}

function isStale(
  binding: ProjectWorkflowBindingView | null | undefined,
  catalog: RecipeViewModel[],
): boolean {
  return Boolean(
    binding
      && (!binding.available
        || !catalog.some((recipe) => (
          recipe.workflowVersionId === binding.workflowVersionId
          && recipe.recipeId === binding.recipeId
        ))),
  );
}

function ConfiguredSelect({
  label,
  value,
  candidates,
  disabled,
  onChange,
  onClear,
}: {
  label: string;
  value: SelectedRecipeRef | undefined;
  candidates: RecipeViewModel[];
  disabled: boolean;
  onChange: (ref: SelectedRecipeRef | undefined) => void;
  onClear: () => void;
}) {
  return (
    <label className="project-workflow-select-row">
      <span>{label}</span>
      <div className="project-workflow-select-controls">
        <select
          aria-label={label}
          value={refKey(value)}
          onChange={(event) => onChange(parseRef(event.target.value))}
          disabled={disabled || !candidates.length}
        >
          <option value={UNSET}>未设置</option>
          {value && !candidates.some((recipe) => refKey(recipe) === refKey(value)) && (
            <option value={refKey(value)}>当前绑定不可用</option>
          )}
          {candidates.map((recipe) => (
            <option key={refKey(recipe)} value={refKey(recipe)}>{recipeLabel(recipe)}</option>
          ))}
        </select>
        {value && <button type="button" className="quiet-button" onClick={onClear} disabled={disabled}>清除绑定</button>}
      </div>
    </label>
  );
}

export function ProjectWorkflowSettings({ projectId, catalog, onConfigChanged }: Props) {
  const [config, setConfig] = useState<ProjectWorkflowConfigView>();
  const [draft, setDraft] = useState<Draft>({ videoModeOverrides: {} });
  const [legacyImage, setLegacyImage] = useState<SelectedRecipeRef>();
  const [legacyVideo, setLegacyVideo] = useState<SelectedRecipeRef>();
  const [legacyOverrides, setLegacyOverrides] = useState<Partial<Record<H3CompatibleMode, SelectedRecipeRef>>>({});
  const [legacyDismissed, setLegacyDismissed] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();

  const imageCandidates = useMemo(() => filterImageRecipes(catalog), [catalog]);
  const videoCandidates = useMemo(() => filterVideoRecipes(catalog), [catalog]);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setError(undefined);
    setNotice(undefined);
    setLegacyDismissed(false);
    void getProjectWorkflowConfig(projectId)
      .then((nextConfig) => {
        if (!active) return;
        setConfig(nextConfig);
        onConfigChanged?.(nextConfig);
        setDraft(draftFromConfig(nextConfig));
        if (configIsEmpty(nextConfig)) {
          setLegacyImage(readSelectedRecipeRef(projectId, "image"));
          setLegacyVideo(readSelectedRecipeRef(projectId, "video"));
          setLegacyOverrides(readProjectWorkflowOverrides(projectId));
        } else {
          setLegacyImage(undefined);
          setLegacyVideo(undefined);
          setLegacyOverrides({});
        }
      })
      .catch((value) => {
        if (active) setError(toUserMessage(value));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => { active = false; };
  }, [onConfigChanged, projectId]);

  function setDraftValue(patch: Partial<Draft>) {
    setDraft((current) => ({ ...current, ...patch }));
    setNotice(undefined);
  }

  async function save(bindings = draftBindings(draft), message = "工作流配置已保存。") {
    setSaving(true);
    setError(undefined);
    setNotice(undefined);
    try {
      const nextConfig = await replaceProjectWorkflowConfig(projectId, { bindings });
      setConfig(nextConfig);
      onConfigChanged?.(nextConfig);
      setDraft(draftFromConfig(nextConfig));
      setLegacyImage(undefined);
      setLegacyVideo(undefined);
      setLegacyOverrides({});
      clearSelectedRecipeRef(projectId, "image");
      clearSelectedRecipeRef(projectId, "video");
      clearProjectWorkflowOverrides(projectId);
      setNotice(message);
    } catch (value) {
      setError(toUserMessage(value));
    } finally {
      setSaving(false);
    }
  }

  function importLegacy() {
    const bindings = [
      bindingInput("IMAGE", "DEFAULT", legacyImage),
      bindingInput("VIDEO", "DEFAULT", legacyVideo),
      ...VIDEO_MODES.map((mode) => bindingInput("VIDEO", mode, legacyOverrides[mode])),
    ].filter((binding): binding is ProjectWorkflowBindingInput => Boolean(binding));
    if (!bindings.length) return;
    void save(bindings, "旧版工作流选择已导入项目配置。");
  }

  if (loading) {
    return <section className="project-workflow-settings" aria-label="项目工作流设置"><p className="disabled-note">正在读取项目工作流配置…</p></section>;
  }
  if (!config) {
    return <section className="project-workflow-settings" aria-label="项目工作流设置"><p className="error-message" role="alert">{error ?? "项目工作流配置读取失败。"}</p></section>;
  }

  const imageStale = isStale(config.imageDefault, imageCandidates);
  const videoStale = isStale(config.videoDefault, videoCandidates);
  const legacyAvailable = Boolean(legacyImage || legacyVideo || Object.keys(legacyOverrides).length);

  return (
    <section className="project-workflow-settings" aria-labelledby="project-workflow-settings-title">
      <div className="section-heading">
        <div>
          <span className="section-label">项目级默认</span>
          <h3 id="project-workflow-settings-title">项目工作流设置</h3>
          <p className="section-description">为当前项目固定图片默认、视频默认和视频模式覆盖；设置只在点击保存后生效。</p>
        </div>
      </div>
      {error && <p className="error-message" role="alert">{error}</p>}
      {notice && <p className="settings-notice" role="status">{notice}</p>}
      {legacyAvailable && configIsEmpty(config) && !legacyDismissed && (
        <div className="project-workflow-migration" role="status">
          <span>检测到旧版本保存的工作流选择</span>
          <div>
            <button type="button" onClick={importLegacy} disabled={saving}>导入旧设置</button>
            <button type="button" className="quiet-button" onClick={() => setLegacyDismissed(true)} disabled={saving}>忽略</button>
          </div>
        </div>
      )}
      {imageStale && <p className="settings-warning" role="alert">⚠ 当前绑定工作流不可用。原 WorkflowVersion：{config.imageDefault?.workflowVersionId} · 原 Recipe：{config.imageDefault?.recipeId}。请重新选择或清除绑定；系统不会静默改写项目配置。</p>}
      {videoStale && <p className="settings-warning" role="alert">⚠ 当前绑定工作流不可用。原 WorkflowVersion：{config.videoDefault?.workflowVersionId} · 原 Recipe：{config.videoDefault?.recipeId}。请重新选择或清除绑定；系统不会静默改写项目配置。</p>}
      <div className="project-workflow-defaults">
        <ConfiguredSelect
          label="图片默认工作流"
          value={draft.imageDefault}
          candidates={imageCandidates}
          disabled={saving}
          onChange={(value) => setDraftValue({ imageDefault: value })}
          onClear={() => setDraftValue({ imageDefault: undefined })}
        />
        <ConfiguredSelect
          label="视频默认工作流"
          value={draft.videoDefault}
          candidates={videoCandidates}
          disabled={saving}
          onChange={(value) => setDraftValue({ videoDefault: value })}
          onClear={() => setDraftValue({ videoDefault: undefined })}
        />
      </div>
      <details className="project-workflow-advanced">
        <summary>高级：按视频模式覆盖</summary>
        <div className="project-workflow-mode-grid">
          {VIDEO_MODES.map((mode) => {
            const configuredBinding = config.videoModeOverrides.find((binding) => binding.mode === mode);
            const candidates = recipesForVideoMode(catalog, mode);
            return (
              <div className="project-workflow-mode-row" key={mode}>
                {isStale(configuredBinding, candidates) && <p className="settings-warning">⚠ {VIDEO_MODE_LABELS[mode]}绑定不可用。原 WorkflowVersion：{configuredBinding?.workflowVersionId} · 原 Recipe：{configuredBinding?.recipeId}。请重新选择或清除。</p>}
                <ConfiguredSelect
                  label={VIDEO_MODE_LABELS[mode]}
                  value={draft.videoModeOverrides[mode]}
                  candidates={candidates}
                  disabled={saving}
                  onChange={(value) => setDraftValue({ videoModeOverrides: { ...draft.videoModeOverrides, [mode]: value } })}
                  onClear={() => {
                    const next = { ...draft.videoModeOverrides };
                    delete next[mode];
                    setDraftValue({ videoModeOverrides: next });
                  }}
                />
              </div>
            );
          })}
        </div>
      </details>
      <div className="project-workflow-settings-actions">
        <button type="button" className="primary-action" onClick={() => void save()} disabled={saving}>
          {saving ? "正在保存…" : "保存工作流配置"}
        </button>
        <button type="button" className="quiet-button" onClick={() => { setDraft({ videoModeOverrides: {} }); setNotice(undefined); }} disabled={saving}>
          恢复未设置
        </button>
      </div>
    </section>
  );
}
