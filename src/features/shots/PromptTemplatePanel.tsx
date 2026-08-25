import { useEffect, useMemo, useState } from "react";
import { applyPromptTemplate, previewPromptTemplate } from "../../services/tauriClient";
import type { ProductionEpisode, ProductionScene, ProductionSeries } from "../../types/productionStructure";
import type { PromptEntryView, PromptVersionView } from "../../types/prompt";
import type { PromptTemplatePreview } from "../../types/promptTemplate";
import type { ReferenceAnchorView } from "../../types/referenceAnchor";
import type { ShotStage, ShotView } from "../../types/shot";
import { toUserMessage } from "../../i18n/errorMessages";
import { analyzePromptTemplateText, customPromptVariableNames, toggleOrderedPromptSelection } from "../prompts/promptTemplateState";

interface Props {
  projectId: string;
  projectName?: string;
  projectDescription?: string | null;
  stage: ShotStage;
  entry: PromptEntryView;
  version: PromptVersionView;
  shot: ShotView;
  structureContext?: {
    series: ProductionSeries;
    episode: ProductionEpisode;
    scene: ProductionScene;
  };
  referenceAnchors: ReferenceAnchorView[];
  onApplied: () => void | Promise<void>;
  disabled?: boolean;
}

const anchorKindLabels: Record<ReferenceAnchorView["kind"], string> = {
  CHARACTER: "角色",
  SCENE: "场景",
  PROP: "道具",
  STYLE: "风格",
};

function stageLabel(stage: ShotStage): string {
  return stage === "image" ? "图片" : "视频";
}

function defaultContextRows(
  projectId: string,
  projectName: string | undefined,
  projectDescription: string | null | undefined,
  shot: ShotView,
  structureContext: Props["structureContext"],
): Array<[string, string]> {
  return [
    ["project.name", projectName || projectId],
    ["project.id", projectId],
    ["project.description", projectDescription || ""],
    ["series.name", structureContext?.series.name || "—"],
    ["episode.name", structureContext?.episode.name || "—"],
    ["scene.name", structureContext?.scene.name || "—"],
    ["shot.name", shot.name],
    ["shot.number", String(shot.ordinal + 1)],
  ];
}

function flattenContext(value: unknown, prefix = ""): Array<[string, string]> {
  if (value === null || value === undefined) return prefix ? [[prefix, ""]] : [];
  if (typeof value !== "object") return prefix ? [[prefix, String(value)]] : [];
  const rows: Array<[string, string]> = [];
  for (const [key, child] of Object.entries(value)) {
    rows.push(...flattenContext(child, prefix ? prefix + "." + key : key));
  }
  return rows;
}

export function PromptTemplatePanel({
  projectId,
  projectName,
  projectDescription,
  stage,
  entry,
  version,
  shot,
  structureContext,
  referenceAnchors,
  onApplied,
  disabled = false,
}: Props) {
  const analysis = useMemo(() => analyzePromptTemplateText(version.text), [version.text]);
  const customNames = useMemo(() => customPromptVariableNames(analysis.customVariables), [analysis.customVariables]);
  const [anchorIds, setAnchorIds] = useState<string[]>([]);
  const [customValues, setCustomValues] = useState<Record<string, string>>({});
  const [preview, setPreview] = useState<PromptTemplatePreview>();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();

  useEffect(() => {
    setAnchorIds([]);
    setCustomValues(Object.fromEntries(customNames.map((name) => [name, ""])));
    setPreview(undefined);
    setError(undefined);
    setNotice(undefined);
  }, [version.id, customNames]);

  const contextRows = preview
    ? flattenContext(preview.context)
    : defaultContextRows(projectId, projectName, projectDescription, shot, structureContext);

  function invalidatePreview() {
    setPreview(undefined);
    setError(undefined);
    setNotice(undefined);
  }

  function changeAnchor(anchorId: string, checked: boolean) {
    setAnchorIds((current) => toggleOrderedPromptSelection(current, anchorId, checked));
    invalidatePreview();
  }

  function changeCustomValue(name: string, value: string) {
    setCustomValues((current) => ({ ...current, [name]: value }));
    invalidatePreview();
  }

  async function previewTemplate() {
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    try {
      setPreview(await previewPromptTemplate({
        projectId,
        promptEntryId: entry.id,
        promptVersionId: version.id,
        shotId: shot.id,
        contextAnchorIds: anchorIds,
        customValues,
      }));
    } catch (previewError: unknown) {
      setPreview(undefined);
      setError(toUserMessage(previewError));
    } finally {
      setBusy(false);
    }
  }

  async function applyTemplate(shotIds: string[], successMessage: string) {
    if (!preview || !shotIds.length) return;
    setBusy(true);
    setError(undefined);
    setNotice(undefined);
    try {
      await applyPromptTemplate({
        projectId,
        promptEntryId: entry.id,
        promptVersionId: version.id,
        stage,
        shotIds,
        contextAnchorIds: anchorIds,
        customValues,
      });
      await onApplied();
      setNotice(successMessage);
    } catch (applyError: unknown) {
      setError(toUserMessage(applyError));
    } finally {
      setBusy(false);
    }
  }

  async function applyCurrentShot() {
    await applyTemplate([shot.id], "模板已渲染并冻结到当前镜头。");
  }

  async function applyCurrentScene() {
    if (!structureContext?.scene.shotIds.length) return;
    const message = "将模板渲染并冻结到场景「" + structureContext.scene.name + "」的 " + structureContext.scene.shotIds.length + " 个镜头，是否继续？";
    if (!window.confirm(message)) return;
    await applyTemplate(structureContext.scene.shotIds, "模板已渲染并冻结到场景「" + structureContext.scene.name + "」。");
  }

  return (
    <section className="prompt-template-panel" aria-label="模板预览">
      <div className="prompt-template-heading">
        <div>
          <span className="section-label">提示词模板</span>
          <h4>模板预览 <em className="prompt-template-badge">提示词模板</em></h4>
          <p>{entry.name} · v{version.version} · 目标 {stageLabel(stage)} · 当前镜头：{shot.name}</p>
        </div>
        <span className="prompt-template-variable-count">{analysis.variables.length} 个变量</span>
      </div>

      <div className="prompt-template-context">
        <div className="prompt-template-subheading"><strong>上下文</strong><small>当前镜头自动解析的上下文</small></div>
        <div className="prompt-template-context-grid">
          {contextRows.map(([key, value]) => <div key={key}><span>{key}</span><strong>{value || "—"}</strong></div>)}
        </div>
      </div>

      <div className="prompt-template-anchor-picker">
        <div className="prompt-template-subheading"><strong>参考锚点</strong><small>仅作为本次模板上下文，不改变素材关系；选择顺序会随本次操作保留。</small></div>
        <div className="prompt-template-anchor-list">
          {referenceAnchors.map((anchor) => (
            <label key={anchor.id}>
              <input type="checkbox" checked={anchorIds.includes(anchor.id)} onChange={(event) => changeAnchor(anchor.id, event.target.checked)} disabled={busy || disabled} />
              <span>[{anchorKindLabels[anchor.kind]}] {anchor.name}</span>
              <small>{anchor.description || "无描述"}</small>
            </label>
          ))}
          {!referenceAnchors.length && <span className="empty-state">当前项目暂无参考锚点。</span>}
        </div>
      </div>

      {customNames.length > 0 && <div className="prompt-template-custom-inputs">
        <div className="prompt-template-subheading"><strong>自定义输入</strong><small>模板自动识别出的自定义变量</small></div>
        <div className="prompt-template-custom-grid">
          {customNames.map((name) => <label key={name}><span>{name}</span><input value={customValues[name] ?? ""} maxLength={4096} onChange={(event) => changeCustomValue(name, event.target.value)} placeholder={"输入 " + name} disabled={busy || disabled} /></label>)}
        </div>
      </div>}

      <div className="prompt-template-actions">
        <button type="button" onClick={() => void previewTemplate()} disabled={busy || disabled}>{busy ? "正在处理…" : "预览模板"}</button>
        <button type="button" className="quiet-button" onClick={() => void applyCurrentShot()} disabled={busy || disabled || !preview}>应用到当前镜头</button>
        {structureContext?.scene && <button type="button" className="quiet-button" onClick={() => void applyCurrentScene()} disabled={busy || disabled || !preview}>应用到当前场景（{structureContext.scene.shotIds.length}）</button>}
      </div>
      {!structureContext?.scene && <p className="shot-inline-note">当前镜头未归档到场景，因此不会显示场景批量应用。</p>}
      {preview && <div className="prompt-template-preview-grid">
        <div><strong>模板文本</strong><pre>{preview.templateText}</pre></div>
        <div><strong>渲染结果</strong><pre>{preview.renderedText}</pre></div>
      </div>}
      {preview?.warnings.map((warning) => <p key={warning} className="settings-warning">{warning}</p>)}
      {error && <p className="error-message" role="alert">{error}</p>}
      {notice && <p className="studio-notice" role="status">{notice}</p>}
    </section>
  );
}
