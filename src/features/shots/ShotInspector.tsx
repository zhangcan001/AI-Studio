import { useEffect, useState, type ReactNode } from "react";
import { getAssetMediaUrl, readAssetImage, readAssetThumbnail } from "../../services/tauriClient";
import type { AssetView } from "../../types/asset";
import type { DraftValue, RecipeField, RecipeViewModel } from "../../types/generation";
import type { ShotInputValues, ShotStage } from "../../types/shot";
import "./ShotInspector.css";

export type ShotInspectorTab = "parameters" | "references" | "prompt";

export interface ShotStageDraftLike {
  workflowVersionId: string;
  recipeId: string;
  values: ShotInputValues;
}

export type ScalarRecipeField = Extract<RecipeField, { type: "integer" | "number" | "seed" }>;

export interface ShotReferenceItem {
  assetId: string;
  asset?: AssetView;
  ordinal?: number;
  label?: string;
}

export interface ShotReferenceAnchorOption {
  id: string;
  name: string;
  kind?: string;
  usable?: boolean;
  assets?: AssetView[];
}

export interface ShotPromptLibraryOption {
  id: string;
  name: string;
  versionCount?: number;
}

export interface ShotInspectorProps {
  projectId: string;
  stage: ShotStage;
  currentDraft?: ShotStageDraftLike;
  currentRecipe?: RecipeViewModel;
  stageRecipes?: RecipeViewModel[];
  onRecipeChange?: (recipeId: string) => void;
  onScalarChange?: (field: ScalarRecipeField, value: DraftValue) => void;
  busy?: boolean;
  canGenerate?: boolean;
  onGenerate: () => void | Promise<void>;
  configDirty?: boolean;
  onSave?: () => void | Promise<void>;
  references?: ShotReferenceItem[];
  availableReferences?: AssetView[];
  referenceAnchors?: ShotReferenceAnchorOption[];
  selectedAnchorId?: string;
  onAnchorChange?: (anchorId: string) => void;
  keyframeAsset?: AssetView;
  onReferenceAdd?: (assetId: string) => void;
  onReferenceRemove?: (assetId: string) => void;
  onReferenceMove?: (index: number, delta: -1 | 1) => void;
  onApplyAnchor?: (mode: "append" | "replace") => void | Promise<void>;
  onSaveReferences?: () => void | Promise<void>;
  promptText?: string;
  onPromptChange?: (text: string) => void;
  promptLibrary?: ShotPromptLibraryOption[];
  selectedPromptId?: string;
  onPromptSelect?: (promptId: string) => void;
  onLoadPrompt?: () => void | Promise<void>;
  promptProvenance?: { entryId: string; versionId: string };
  promptPreview?: string;
  promptTemplate?: ReactNode;
  onPreviewPrompt?: () => void | Promise<void>;
  onApplyPrompt?: () => void | Promise<void>;
  activeTab?: ShotInspectorTab;
  onTabChange?: (tab: ShotInspectorTab) => void;
}

const ADVANCED_FIELD_PATTERN = /seed|sampler|denoise|guidance|detail|low[_-]?frequency|high[_-]?frequency|noise/i;

export function ShotInspector({
  projectId,
  stage,
  currentDraft,
  currentRecipe,
  stageRecipes = [],
  onRecipeChange,
  onScalarChange,
  busy = false,
  canGenerate = true,
  onGenerate,
  configDirty = false,
  onSave,
  references = [],
  availableReferences = [],
  referenceAnchors = [],
  selectedAnchorId = "",
  onAnchorChange,
  keyframeAsset,
  onReferenceAdd,
  onReferenceRemove,
  onReferenceMove,
  onApplyAnchor,
  onSaveReferences,
  promptText = "",
  onPromptChange,
  promptLibrary = [],
  selectedPromptId = "",
  onPromptSelect,
  onLoadPrompt,
  promptProvenance,
  promptPreview,
  promptTemplate,
  onPreviewPrompt,
  onApplyPrompt,
  activeTab,
  onTabChange,
}: ShotInspectorProps) {
  const [uncontrolledTab, setUncontrolledTab] = useState<ShotInspectorTab>("parameters");
  const selectedTab = activeTab ?? uncontrolledTab;
  const scalarFields = currentRecipe?.fields.filter(isScalarRecipeField) ?? [];
  const commonFields = scalarFields.filter((field) => !ADVANCED_FIELD_PATTERN.test(field.key));
  const advancedFields = scalarFields.filter((field) => ADVANCED_FIELD_PATTERN.test(field.key));
  const nonScalarFieldCount = (currentRecipe?.fields.length ?? 0) - scalarFields.length;

  function selectTab(tab: ShotInspectorTab) {
    if (activeTab === undefined) setUncontrolledTab(tab);
    onTabChange?.(tab);
  }

  return (
    <aside className="shot-inspector" aria-label="镜头检查器">
      <div className="shot-inspector-heading">
        <div>
          <span className="shot-inspector-kicker">Inspector</span>
          <h2>镜头设置</h2>
        </div>
        <span className="shot-inspector-stage">{stage === "image" ? "图片" : "视频"}</span>
      </div>
      <div className="shot-inspector-actions">
        <button type="button" className="shot-inspector-generate" onClick={() => void onGenerate()} disabled={busy || !canGenerate}>
          {busy ? "生成中…" : "生成"}
        </button>
        {configDirty && onSave && <button type="button" className="quiet-button" onClick={() => void onSave()} disabled={busy}>保存配置</button>}
      </div>
      <div className="shot-inspector-tabs" role="tablist" aria-label="Inspector sections">
        <InspectorTabButton tab="parameters" label="参数" selected={selectedTab === "parameters"} onSelect={selectTab} />
        <InspectorTabButton tab="references" label="参考" selected={selectedTab === "references"} onSelect={selectTab} />
        <InspectorTabButton tab="prompt" label="提示词" selected={selectedTab === "prompt"} onSelect={selectTab} />
      </div>

      {selectedTab === "parameters" && (
        <div className="shot-inspector-panel" role="tabpanel" aria-label="参数">
          <section className="shot-inspector-section">
            <div className="shot-inspector-section-heading"><div><span className="shot-inspector-label">Runtime</span><h3>生成参数</h3></div></div>
            {stageRecipes.length > 0 && (
              <label className="shot-inspector-field">
                <span>工作流 / Recipe</span>
                <select value={currentDraft?.recipeId ?? ""} onChange={(event) => onRecipeChange?.(event.target.value)} disabled={busy || !onRecipeChange}>
                  <option value="">选择兼容 Recipe</option>
                  {stageRecipes.map((recipe) => <option key={`${recipe.workflowVersionId}:${recipe.recipeId}`} value={recipe.recipeId}>{recipe.name} · {recipe.mode}</option>)}
                </select>
              </label>
            )}
            {currentRecipe ? (
              <div className="shot-inspector-runtime-meta" aria-label="当前工作流信息">
                <span><small>Workflow</small><strong>{currentRecipe.name}</strong></span>
                <span><small>Recipe</small><strong>{currentRecipe.recipeId}</strong></span>
              </div>
            ) : (
              <div className="shot-inspector-empty"><strong>尚未配置阶段参数</strong><span>选择一个兼容 Recipe 后即可编辑生成参数。</span></div>
            )}
            {commonFields.map((field) => <ScalarField key={field.key} field={field} value={currentDraft?.values[field.key]} disabled={busy || !onScalarChange} onChange={(value) => onScalarChange?.(field, value)} />)}
            {advancedFields.length > 0 && (
              <details className="shot-inspector-advanced">
                <summary><span>高级设置</span><small>{advancedFields.length} 个参数</small></summary>
                <div className="shot-inspector-advanced-fields">
                  {advancedFields.map((field) => <ScalarField key={field.key} field={field} value={currentDraft?.values[field.key]} disabled={busy || !onScalarChange} onChange={(value) => onScalarChange?.(field, value)} />)}
                </div>
              </details>
            )}
            {nonScalarFieldCount > 0 && <p className="shot-inspector-note">{nonScalarFieldCount} 个素材输入由“参考”面板维护，不在这里重复编辑。</p>}
          </section>
        </div>
      )}

      {selectedTab === "references" && (
        <ReferenceInspector
          projectId={projectId}
          stage={stage}
          references={references}
          availableReferences={availableReferences}
          referenceAnchors={referenceAnchors}
          selectedAnchorId={selectedAnchorId}
          onAnchorChange={onAnchorChange}
          keyframeAsset={keyframeAsset}
          onReferenceAdd={onReferenceAdd}
          onReferenceRemove={onReferenceRemove}
          onReferenceMove={onReferenceMove}
          onApplyAnchor={onApplyAnchor}
          onSaveReferences={onSaveReferences}
          busy={busy}
        />
      )}

      {selectedTab === "prompt" && (
        <PromptInspector
          promptText={promptText}
          onPromptChange={onPromptChange}
          promptLibrary={promptLibrary}
          selectedPromptId={selectedPromptId}
          onPromptSelect={onPromptSelect}
          onLoadPrompt={onLoadPrompt}
          promptProvenance={promptProvenance}
          promptPreview={promptPreview}
          promptTemplate={promptTemplate}
          onPreviewPrompt={onPreviewPrompt}
          onApplyPrompt={onApplyPrompt}
          busy={busy}
        />
      )}
    </aside>
  );
}

function InspectorTabButton({ tab, label, selected, onSelect }: { tab: ShotInspectorTab; label: string; selected: boolean; onSelect: (tab: ShotInspectorTab) => void }) {
  return <button type="button" role="tab" aria-selected={selected} className={selected ? "shot-inspector-tab shot-inspector-tab-active" : "shot-inspector-tab"} onClick={() => onSelect(tab)}>{label}</button>;
}

function isScalarRecipeField(field: RecipeField): field is ScalarRecipeField {
  return field.type === "integer" || field.type === "number" || field.type === "seed";
}

function ScalarField({ field, value, disabled, onChange }: { field: ScalarRecipeField; value?: DraftValue; disabled: boolean; onChange: (value: DraftValue) => void }) {
  if (field.type === "seed") {
    const fixed = value?.type === "seed_fixed";
    return (
      <label className="shot-inspector-field">
        <span>{field.label}</span>
        <span className="shot-inspector-seed-row">
          <select value={fixed ? "fixed" : "random"} onChange={(event) => onChange(event.target.value === "fixed" ? { type: "seed_fixed", value: field.defaultValue ?? "0" } : { type: "seed_random" })} disabled={disabled}>
            <option value="random">随机</option>
            <option value="fixed">固定</option>
          </select>
          {fixed && <input value={value.value} inputMode="numeric" onChange={(event) => onChange({ type: "seed_fixed", value: event.target.value })} disabled={disabled} aria-label={`${field.label}值`} />}
        </span>
      </label>
    );
  }
  const valueType = field.type === "integer" ? "integer" : "number";
  const currentValue = value?.type === valueType ? value.value : "";
  return (
    <label className="shot-inspector-field">
      <span>{field.label}</span>
      <input type="number" value={currentValue} min={field.min} max={field.max} step={field.step ?? (field.type === "integer" ? 1 : "any")} onChange={(event) => onChange({ type: valueType, value: Number(event.target.value) })} disabled={disabled} />
    </label>
  );
}

function ReferenceInspector({ projectId, stage, references, availableReferences, referenceAnchors, selectedAnchorId, onAnchorChange, keyframeAsset, onReferenceAdd, onReferenceRemove, onReferenceMove, onApplyAnchor, onSaveReferences, busy }: {
  projectId: string;
  stage: ShotStage;
  references: ShotReferenceItem[];
  availableReferences: AssetView[];
  referenceAnchors: ShotReferenceAnchorOption[];
  selectedAnchorId: string;
  onAnchorChange?: (anchorId: string) => void;
  keyframeAsset?: AssetView;
  onReferenceAdd?: (assetId: string) => void;
  onReferenceRemove?: (assetId: string) => void;
  onReferenceMove?: (index: number, delta: -1 | 1) => void;
  onApplyAnchor?: (mode: "append" | "replace") => void | Promise<void>;
  onSaveReferences?: () => void | Promise<void>;
  busy: boolean;
}) {
  const selectedAnchor = referenceAnchors.find((anchor) => anchor.id === selectedAnchorId);
  const listedIds = new Set(references.map((reference) => reference.assetId));
  return (
    <div className="shot-inspector-panel" role="tabpanel" aria-label="参考">
      <section className="shot-inspector-section">
        <div className="shot-inspector-section-heading"><div><span className="shot-inspector-label">References</span><h3>{stage === "video" ? "有序参考图" : "参考素材"}</h3></div>{onSaveReferences && <button type="button" className="quiet-button" onClick={() => void onSaveReferences()} disabled={busy}>保存</button>}</div>
        {stage === "video" && keyframeAsset && <div className="shot-inspector-keyframe"><AssetThumb projectId={projectId} asset={keyframeAsset} /><span><small>关键帧</small><strong>{keyframeAsset.name}</strong></span></div>}
        {referenceAnchors.length > 0 && (
          <div className="shot-inspector-anchor-picker">
            <label className="shot-inspector-field"><span>Reference Anchor</span><select value={selectedAnchorId} onChange={(event) => onAnchorChange?.(event.target.value)} disabled={busy || !onAnchorChange}><option value="">选择参考锚点</option>{referenceAnchors.map((anchor) => <option key={anchor.id} value={anchor.id} disabled={anchor.usable === false}>{anchor.kind ? `[${anchor.kind}] ` : ""}{anchor.name}</option>)}</select></label>
            <div className="shot-inspector-inline-actions">
              <button type="button" className="quiet-button" onClick={() => onApplyAnchor?.("append")} disabled={busy || !selectedAnchor || selectedAnchor.usable === false || !onApplyAnchor}>追加 Anchor</button>
              <button type="button" className="quiet-button" onClick={() => onApplyAnchor?.("replace")} disabled={busy || !selectedAnchor || selectedAnchor.usable === false || !onApplyAnchor}>替换 Anchor</button>
            </div>
          </div>
        )}
        <div className="shot-inspector-reference-list">
          {references.map((reference, index) => (
            <div className="shot-inspector-reference-row" key={`${reference.assetId}:${index}`}>
              <span className="shot-inspector-reference-index">@图片{index + 1}</span>
              {reference.asset ? <AssetThumb projectId={projectId} asset={reference.asset} /> : <span className="shot-inspector-thumb-placeholder">素材</span>}
              <span className="shot-inspector-reference-copy"><strong>{reference.label ?? reference.asset?.name ?? reference.assetId}</strong><small>{reference.assetId}</small></span>
              <div className="shot-inspector-reference-actions">
                <button type="button" className="quiet-button" aria-label={`上移 ${reference.label ?? reference.asset?.name ?? reference.assetId}`} onClick={() => onReferenceMove?.(index, -1)} disabled={busy || !onReferenceMove || index === 0}>↑</button>
                <button type="button" className="quiet-button" aria-label={`下移 ${reference.label ?? reference.asset?.name ?? reference.assetId}`} onClick={() => onReferenceMove?.(index, 1)} disabled={busy || !onReferenceMove || index === references.length - 1}>↓</button>
                <button type="button" className="quiet-button" onClick={() => onReferenceRemove?.(reference.assetId)} disabled={busy || !onReferenceRemove}>移除</button>
              </div>
            </div>
          ))}
          {references.length === 0 && <EmptyInspectorState title="暂无参考素材" detail="从下方素材中添加，或选择一个 Reference Anchor。" />}
        </div>
      </section>
      <section className="shot-inspector-section">
        <div className="shot-inspector-section-heading"><div><span className="shot-inspector-label">Available</span><h3>可用素材</h3></div></div>
        <div className="shot-inspector-available-list">
          {availableReferences.filter((asset) => !listedIds.has(asset.id)).slice(0, 18).map((asset) => <button key={asset.id} type="button" className="shot-inspector-available-item" onClick={() => onReferenceAdd?.(asset.id)} disabled={busy || !onReferenceAdd}><AssetThumb projectId={projectId} asset={asset} /><span><strong>{asset.name}</strong><small>{asset.id}</small></span><b aria-hidden="true">＋</b></button>)}
          {availableReferences.filter((asset) => !listedIds.has(asset.id)).length === 0 && <p className="shot-inspector-note">没有可追加的素材。</p>}
        </div>
      </section>
    </div>
  );
}

function PromptInspector({ promptText, onPromptChange, promptLibrary, selectedPromptId, onPromptSelect, onLoadPrompt, promptProvenance, promptPreview, promptTemplate, onPreviewPrompt, onApplyPrompt, busy }: {
  promptText: string;
  onPromptChange?: (text: string) => void;
  promptLibrary: ShotPromptLibraryOption[];
  selectedPromptId: string;
  onPromptSelect?: (promptId: string) => void;
  onLoadPrompt?: () => void | Promise<void>;
  promptProvenance?: { entryId: string; versionId: string };
  promptPreview?: string;
  promptTemplate?: ReactNode;
  onPreviewPrompt?: () => void | Promise<void>;
  onApplyPrompt?: () => void | Promise<void>;
  busy: boolean;
}) {
  return (
    <div className="shot-inspector-panel" role="tabpanel" aria-label="提示词">
      <section className="shot-inspector-section">
        <div className="shot-inspector-section-heading"><div><span className="shot-inspector-label">Prompt</span><h3>当前提示词</h3></div></div>
        <label className="shot-inspector-field"><span>Prompt Text</span><textarea value={promptText} onChange={(event) => onPromptChange?.(event.target.value)} disabled={busy || !onPromptChange} rows={8} placeholder="描述镜头画面、动作和构图" /></label>
        {promptProvenance && <p className="shot-inspector-note">来源：Prompt Library · version {promptProvenance.versionId.slice(-8)}</p>}
      </section>
      {promptLibrary.length > 0 && (
        <section className="shot-inspector-section">
          <div className="shot-inspector-section-heading"><div><span className="shot-inspector-label">Library</span><h3>Prompt Library</h3></div></div>
          <div className="shot-inspector-prompt-loader"><select value={selectedPromptId} onChange={(event) => onPromptSelect?.(event.target.value)} disabled={busy || !onPromptSelect}><option value="">选择提示词</option>{promptLibrary.map((prompt) => <option key={prompt.id} value={prompt.id}>{prompt.name}{prompt.versionCount ? ` · ${prompt.versionCount} 版` : ""}</option>)}</select><button type="button" className="quiet-button" onClick={() => void onLoadPrompt?.()} disabled={busy || !selectedPromptId || !onLoadPrompt}>载入快照</button></div>
        </section>
      )}
      {promptTemplate && <section className="shot-inspector-section shot-inspector-template-slot"><span className="shot-inspector-label">Template</span>{promptTemplate}</section>}
      <section className="shot-inspector-section">
        <div className="shot-inspector-section-heading"><div><span className="shot-inspector-label">Preview</span><h3>Prompt Preview</h3></div></div>
        <pre className="shot-inspector-prompt-preview">{promptPreview || promptText || "尚无 Prompt 预览。"}</pre>
        <div className="shot-inspector-inline-actions"><button type="button" className="quiet-button" onClick={() => void onPreviewPrompt?.()} disabled={busy || !onPreviewPrompt}>预览</button><button type="button" onClick={() => void onApplyPrompt?.()} disabled={busy || !onApplyPrompt || !(promptPreview ?? promptText).trim()}>应用 Prompt</button></div>
      </section>
    </div>
  );
}

function AssetThumb({ projectId, asset }: { projectId: string; asset: AssetView }) {
  const isVideo = asset.assetType === "video" || asset.category === "source_video" || asset.category === "generated_video";
  const isAudio = asset.assetType === "audio" || asset.category === "source_audio";
  const previewUrl = useAssetThumbnailUrl(projectId, asset);
  const mediaUrl = isVideo || isAudio ? getAssetMediaUrl(projectId, asset.id, isVideo ? "video" : "audio") : undefined;
  return <span className="shot-inspector-thumb">{previewUrl ? <img src={previewUrl} alt={asset.name} loading="lazy" /> : mediaUrl && isVideo ? <video src={mediaUrl} aria-label={asset.name} preload="metadata" muted playsInline /> : <span>{isAudio ? "音频" : isVideo ? "视频" : "图片"}</span>}</span>;
}

function useAssetThumbnailUrl(projectId: string, asset: AssetView): string | undefined {
  const [url, setUrl] = useState<string>();
  const isVideo = asset.assetType === "video" || asset.category === "source_video" || asset.category === "generated_video";
  useEffect(() => {
    let active = true;
    let objectUrl: string | undefined;
    setUrl(undefined);
    const read = asset.thumbnailAvailable
      ? readAssetThumbnail(projectId, asset.id).catch(() => readAssetImage(projectId, asset.id))
      : isVideo
        ? undefined
        : readAssetImage(projectId, asset.id);
    if (!read) return () => { active = false; };
    void read.then((bytes) => {
      if (!active) return;
      objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
      setUrl(objectUrl);
    }).catch(() => undefined);
    return () => {
      active = false;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [asset.id, asset.mimeType, asset.thumbnailAvailable, asset.assetType, asset.category, isVideo, projectId]);
  return url;
}

function EmptyInspectorState({ title, detail }: { title: string; detail: string }) {
  return <div className="shot-inspector-empty"><strong>{title}</strong><span>{detail}</span></div>;
}
