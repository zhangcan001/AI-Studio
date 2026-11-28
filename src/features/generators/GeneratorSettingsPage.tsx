import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { GENERATOR_BINDINGS_CHANGED, LAST_GENERATOR_SESSION_KEY, type GeneratorBindingSummary, type GeneratorOption } from "../../product/types";
import type { AppRoute } from "../../app/routes/types";
import { ThemedConfirmDialog } from "../../app/WorkspaceErrorBoundary";
import { generatorLabel, fieldLabel, currentWorkflowRuntime } from "../../product/generatorPresentation";
import { customWorkflowTransport, type WorkflowWorkspaceQueryResponse } from "../../product/transport";

import type {
  WorkflowAutoIssueCandidateView,
  WorkflowAutoIssueView,
  WorkflowAutoOnboardingPlanView,
  WorkflowFieldType,
} from "../../types/workflowOnboarding";
import "./GeneratorSettingsPage.css";

const { commitWorkflowImport, discardOnboarding, importWorkflowFromPath, pickApiWorkflow, queryWorkflowWorkspace, reanalyzeWorkflowImport, removeWorkflow, restoreWorkflow, setOnboardingInputMapping, setOnboardingMetadata, setOnboardingOutputMapping, setWorkflowEnabled } = customWorkflowTransport;

const semanticFieldLabels: Record<string, string> = {
  prompt: "正向提示词",
  negative_prompt: "反向提示词",
  image: "输入图片",
  first_frame: "首帧图片",
  last_frame: "尾帧图片",
  reference_image: "参考图片",
  reference_images: "参考图片集",
  reference_video: "参考视频",
  width: "宽度",
  height: "高度",
  duration_seconds: "时长（秒）",
  fps: "帧率",
  seed: "随机种子",
  steps: "采样步数",
  cfg: "CFG 引导",
  denoise: "重绘幅度",
};

function friendlyCategory(category?: string): string {
  if (category === "video") return "视频";
  if (category === "image") return "图片";
  return "待确认";
}

function friendlyMode(mode?: string): string {
  const m = (mode ?? "").toLowerCase();
  if (m.includes("first_last") || m.includes("fl2v")) return "首尾帧视频";
  if (m.includes("reference") || m.includes("ref2v")) return "参考图视频";
  if (m.includes("image_to_video") || m === "i2v") return "图生视频";
  if (m.includes("text_to_video") || m === "t2v") return "文生视频";
  if (m.includes("image_to_image") || m === "i2i") return "图生图";
  if (m.includes("text_to_image") || m === "t2i") return "文生图";
  return "自定义模式";
}

function notifyBindingsChanged(projectId: string) {
  try {
    sessionStorage.removeItem(`${LAST_GENERATOR_SESSION_KEY}:${projectId}:video`);
  } catch { /* storage unavailable */ }
  window.dispatchEvent(new CustomEvent(GENERATOR_BINDINGS_CHANGED, { detail: { projectId } }));
}

const bindingModeLabels: Record<string, string> = { FL2VA_TEXT_TO_VIDEO: "文生视频", FL2VA_IMAGE_TO_VIDEO: "图生视频", FL2VA_FIRST_LAST: "首尾帧视频", REF2VA_IMAGE: "参考图视频", REF2VA_VIDEO_IMAGE: "参考视频与参考图", REF2VA_AUDIO: "历史参考音频模式", REF2VA_IMAGE_AUDIO: "历史图音频模式" };
// Presentation compatibility only: binding slot vocabulary is not Recipe.mode.
// Unknown modes remain selectable only as defaults; never invent a binding slot.
function bindingModes(option: GeneratorOption): string[] {
  const mode = option.mode.toUpperCase();
  if (bindingModeLabels[mode]) return [mode];
  if (mode !== "REF2VA") return [];
  const image = option.fields.some(f => f.type === "image" || f.type === "images");
  const video = option.fields.some(f => f.type === "video" || f.type === "videos");
  return [...(image ? ["REF2VA_IMAGE"] : []), ...(image && video ? ["REF2VA_VIDEO_IMAGE"] : [])];
}

export function GeneratorSettingsPage({ projectId, navigate, onDirtyChange }: { projectId: string; navigate: (route: AppRoute) => unknown; onDirtyChange?: (dirty: boolean) => void }) {
  const [options, setOptions] = useState<GeneratorOption[]>([]);
  const [bindings, setBindings] = useState<GeneratorBindingSummary[]>([]);
  const [choices, setChoices] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [batchError, setBatchError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const [customWorkflows, setCustomWorkflows] = useState<WorkflowWorkspaceQueryResponse["items"]>([]);
  const [importPlan, setImportPlan] = useState<WorkflowAutoOnboardingPlanView>();
  const [importNameDraft, setImportNameDraft] = useState("");
  const [importCategoryDraft, setImportCategoryDraft] = useState("video");
  const [importModeDraft, setImportModeDraft] = useState("image_to_video");
  const [importBusy, setImportBusy] = useState(false);
  const importLock = useRef(false);
  const dropImportRef = useRef<(path: string) => Promise<void>>(async () => {});
  const [importError, setImportError] = useState<string>();
  const [dragOverJson, setDragOverJson] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<{ workflowId: string; name: string }>();
  const generation = useRef(0);
  const readSeq = useRef(0);
  const locked = useRef(false);
  const retainedConflictKeysRef = useRef<Set<string>>(new Set());
  async function loadCustomWorkflows(token: number) {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return;
    try {
      const response = await queryWorkflowWorkspace("FAST");
      if (token !== generation.current || !response?.items) return;
      setCustomWorkflows(response.items.filter(item => item.registry?.sourceKind === "USER" && item.registry.libraryState !== "REMOVED"));
    } catch {
      /* Unit tests without workflow workspace mock simply keep custom list empty */
    }
  }
  async function refresh(token: number, keepChoices = false, seq = ++readSeq.current): Promise<boolean> {
    const [overview, videos] = await Promise.all([
      productClient.project.getOverview(projectId), productClient.creation.generatorsList(projectId, "video"),
    ]);
    if (token !== generation.current || seq !== readSeq.current) return false;
    // No recommendation/name inference is a substitute for a persisted binding.
    if (!overview.generatorBindings) throw new Error("暂时无法读取生成器设置，请刷新。");
    setBindings(overview.generatorBindings);
    setOptions(videos.filter(o => o.mediaKind === "video"));
    if (!keepChoices) setChoices({});
    void loadCustomWorkflows(token);
    return true;
  }
  useEffect(() => {
    const token = ++generation.current;
    const seq = ++readSeq.current;
    retainedConflictKeysRef.current.clear();
    setLoading(true); setError(undefined); setBatchError(undefined); setNotice(undefined); setOptions([]); setBindings([]); setChoices({});
    void refresh(token, false, seq)
      .catch(e => { if (token === generation.current && seq === readSeq.current) setError(normalizeProductError(e).message); })
      .finally(() => { if (token === generation.current && seq === readSeq.current) setLoading(false); });
    return () => { generation.current++; readSeq.current++; };
  }, [projectId]);
  // Image generation is retired from the product: IMAGE bindings stay persisted
  // for back-compat but are no longer offered as a normal project setting.
  const slots = [{ stage: "VIDEO", mode: "DEFAULT", label: "默认视频生成器" },
    ...Array.from(new Set([...options.filter(o => o.mediaKind === "video").flatMap(bindingModes), ...bindings.filter(b => b.stage === "VIDEO").map(b => b.mode)]))
      .filter(mode => mode !== "DEFAULT").map(mode => ({ stage: "VIDEO", mode, label: `${bindingModeLabels[mode] ?? "历史视频模式"}生成器` }))];
  const dirtySlots = slots.flatMap(slot => {
    const key = `${slot.stage}:${slot.mode}`;
    const binding = bindings.find(b => b.stage === slot.stage && b.mode === slot.mode);
    const available = options.filter(o => o.mediaKind === (slot.stage === "IMAGE" ? "image" : "video") && (slot.mode === "DEFAULT" || bindingModes(o).includes(slot.mode)));
    const selected = choices[key];
    if (!selected || selected === binding?.selectionRef) return [];
    const option = available.find(o => o.selectionRef === selected);
    return option?.availability ? [{ ...slot, key, selected, binding }] : [];
  });
  useEffect(() => {
    onDirtyChange?.(dirtySlots.length > 0 || Boolean(importPlan) || importBusy);
    return () => onDirtyChange?.(false);
  }, [dirtySlots.length, importPlan, importBusy, onDirtyChange]);
  async function save(stage: string, mode: string, selectionRef: string) {
    if (locked.current || !selectionRef) return;
    const key = `${stage}:${mode}`;
    locked.current = true; setBusy(true); setError(undefined); setNotice(undefined);
    const retainChoiceOnConflict = Boolean(batchError) || retainedConflictKeysRef.current.has(key);
    setBatchError(undefined);
    const token = generation.current;
    const current = bindings.find(b => b.stage === stage && b.mode === mode);
    let committed = false;
    try {
      await productClient.project.generatorBindingSet(projectId, { stage, mode, selectionRef, expectedRevision: current?.revision ?? null, expectedBindingInstanceId: current?.bindingInstanceId ?? null });
      committed = true;
      retainedConflictKeysRef.current.delete(key);
      notifyBindingsChanged(projectId);
    } catch (e) {
      if (token !== generation.current) {
        locked.current = false;
        return;
      }
      const failure = normalizeProductError(e);
      setError(failure.message);
      // Refresh after a conflict, but never retry or silently overwrite it.
      try { await refresh(token, true); } catch { if (token === generation.current) setNotice("刷新失败，请重新打开生成器设置。尚未重试保存。"); }
      locked.current = false;
      setBusy(false);
      if (retainChoiceOnConflict) {
        retainedConflictKeysRef.current.add(key);
        return;
      }
      setChoices(current => { const next = { ...current }; delete next[key]; return next; });
      return;
    }
    if (token !== generation.current) {
      locked.current = false;
      return;
    }
    setChoices(current => { const next = { ...current }; delete next[key]; return next; });
    try {
      await refresh(token, true);
      if (token === generation.current) {
        setNotice("生成器设置已保存；已有镜头与历史运行不变。");
        window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "生成器设置已保存", tone: "success" } }));
      }
    } catch (e) {
      if (token === generation.current) {
        setError(normalizeProductError(e).message);
        setNotice("生成器设置已保存，但读取最新设置失败；请点击刷新设置。");
      }
    } finally {
      locked.current = false;
      if (token === generation.current && committed) setBusy(false);
    }
  }
  async function saveAll() {
    if (locked.current || !dirtySlots.length) return;
    locked.current = true; setBusy(true); setError(undefined); setNotice(undefined);
    setBatchError(undefined);
    const token = generation.current;
    let savedCount = 0;
    try {
      for (const item of dirtySlots) {
        if (token !== generation.current) return;
        await productClient.project.generatorBindingSet(projectId, {
          stage: item.stage,
          mode: item.mode,
          selectionRef: item.selected,
          expectedRevision: item.binding?.revision ?? null,
          expectedBindingInstanceId: item.binding?.bindingInstanceId ?? null,
        });
        if (token !== generation.current) return;
        savedCount += 1;
      }
      if (savedCount > 0) notifyBindingsChanged(projectId);
      for (const item of dirtySlots) retainedConflictKeysRef.current.delete(item.key);
      if (token === generation.current) {
        await refresh(token, true);
        if (token === generation.current) {
          setNotice(`已批量保存 ${savedCount} 项生成器设置。`);
          window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: `已保存 ${savedCount} 项生成器设置`, tone: "success" } }));
        }
      }
    } catch (e) {
      if (savedCount > 0) notifyBindingsChanged(projectId);
      if (token !== generation.current) return;
      for (const item of dirtySlots.slice(0, savedCount)) retainedConflictKeysRef.current.delete(item.key);
      for (const item of dirtySlots.slice(savedCount)) retainedConflictKeysRef.current.add(item.key);
      const pending = dirtySlots[savedCount];
      setBatchError(pending
        ? `已保存 ${savedCount} 项。「${pending.label}」保存失败：${normalizeProductError(e).message}。其余修改已保留。`
        : `已保存 ${savedCount} 项，读取最新设置失败：${normalizeProductError(e).message}`);
      try { await refresh(token, true); }
      catch (cause) { if (token === generation.current) setError(normalizeProductError(cause).message); }
    } finally {
      locked.current = false;
      if (token === generation.current) {
        const committed = dirtySlots.slice(0, savedCount);
        setChoices(current => Object.fromEntries(Object.entries(current).filter(([key]) => !committed.some(slot => slot.key === key))));
        setBusy(false);
      }
    }
  }
  async function handleRefresh() {
    if (locked.current || busy || loading) return;
    const token = generation.current;
    const seq = ++readSeq.current;
    locked.current = true;
    setBusy(true);
    try {
      const applied = await refresh(token, true, seq);
      if (applied && token === generation.current && seq === readSeq.current) {
        setError(undefined);
        setBatchError(undefined);
      }
    } catch (e) {
      if (token === generation.current && seq === readSeq.current) setError(normalizeProductError(e).message);
    } finally {
      locked.current = false;
      if (token === generation.current) setBusy(false);
    }
  }
  useEffect(() => {
    if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return;
    let unlisten: (() => void) | undefined;
    let alive = true;
    void import("@tauri-apps/api/webview")
      .then(({ getCurrentWebview }) => getCurrentWebview().onDragDropEvent((event) => {
        if (!alive) return;
        const payload = event.payload;
        if (payload.type === "over" || payload.type === "enter") {
          setDragOverJson(true);
        } else if (payload.type === "leave") {
          setDragOverJson(false);
        } else if (payload.type === "drop") {
          setDragOverJson(false);
          const jsonPath = payload.paths.find(p => p.toLowerCase().endsWith(".json"));
          if (jsonPath) {
            void dropImportRef.current(jsonPath);
          }
        }
      }))
      .then((fn) => {
        if (alive) unlisten = fn;
        else fn();
      })
      .catch(() => { /* Outside Tauri runtime */ });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [projectId]);

  dropImportRef.current = startImportFromPath;

  function applyImportPlan(plan: WorkflowAutoOnboardingPlanView, fallbackName: string) {
    setImportPlan(plan);
    setImportNameDraft(plan.metadata.name || fallbackName);
    setImportCategoryDraft(plan.metadata.category === "image" ? "image" : "video");
    setImportModeDraft(plan.metadata.mode && plan.metadata.mode !== "unknown" ? plan.metadata.mode : "image_to_video");
  }

  async function startImportFromPicker() {
    if (importLock.current || importBusy || importPlan) return;
    importLock.current = true;
    setImportBusy(true);
    setImportError(undefined);
    setNotice(undefined);
    const token = generation.current;
    try {
      const draft = await pickApiWorkflow();
      if (!draft) return;
      if (token !== generation.current) { await discardOnboarding(draft.draftId); return; }
      const plan = await reanalyzeWorkflowImport(draft.draftId);
      if (token !== generation.current) { await discardOnboarding(draft.draftId); return; }
      applyImportPlan(plan, draft.originalFilename.replace(/\.json$/i, ""));
    } catch (cause) {
      setImportError(normalizeProductError(cause).message);
    } finally {
      importLock.current = false;
      setImportBusy(false);
    }
  }

  async function startImportFromPath(filePath: string) {
    if (importLock.current || importBusy || importPlan) return;
    importLock.current = true;
    setImportBusy(true);
    setImportError(undefined);
    setNotice(undefined);
    const token = generation.current;
    try {
      const draft = await importWorkflowFromPath(filePath);
      if (token !== generation.current) { await discardOnboarding(draft.draftId); return; }
      const plan = await reanalyzeWorkflowImport(draft.draftId);
      if (token !== generation.current) { await discardOnboarding(draft.draftId); return; }
      applyImportPlan(plan, draft.originalFilename.replace(/\.json$/i, ""));
    } catch (cause) {
      setImportError(normalizeProductError(cause).message);
    } finally {
      importLock.current = false;
      setImportBusy(false);
    }
  }

  async function retryImportAnalysis() {
    if (!importPlan || importBusy || importLock.current) return;
    importLock.current = true;
    setImportBusy(true);
    setImportError(undefined);
    try {
      const nextPlan = await reanalyzeWorkflowImport(importPlan.draftId);
      applyImportPlan(nextPlan, importNameDraft);
    } catch (cause) {
      setImportError(normalizeProductError(cause).message);
    } finally {
      importLock.current = false;
      setImportBusy(false);
    }
  }

  async function restoreArchivedWorkflow() {
    if (!importPlan?.existingWorkflowId || importBusy || importLock.current) return;
    importLock.current = true;
    setImportBusy(true);
    setImportError(undefined);
    try {
      await restoreWorkflow(importPlan.existingWorkflowId);
      await discardOnboarding(importPlan.draftId).catch(() => { /* ignore */ });
      setImportPlan(undefined);
      setImportNameDraft("");
      notifyBindingsChanged(projectId);
      await refresh(generation.current, true);
      setNotice("已恢复该自定义工作流，现在可在上方生成器下拉框中选用。");
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "已恢复自定义工作流", tone: "success" } }));
    } catch (cause) {
      setImportError(normalizeProductError(cause).message);
    } finally {
      importLock.current = false;
      setImportBusy(false);
    }
  }

  async function resolveImportCandidate(issue: WorkflowAutoIssueView, candidate: WorkflowAutoIssueCandidateView) {
    if (!importPlan || importBusy || importLock.current) return;
    importLock.current = true;
    setImportBusy(true);
    setImportError(undefined);
    try {
      if (candidate.outputId && candidate.outputType && candidate.nodeId) {
        const outputKind: "image" | "video" | "audio" =
          candidate.outputType === "image" || candidate.outputType === "audio" ? candidate.outputType : "video";
        await setOnboardingOutputMapping(importPlan.draftId, {
          outputId: candidate.outputId,
          type: outputKind,
          nodeId: candidate.nodeId,
          label: candidate.label,
          required: true,
        });
      } else if (issue.field && candidate.nodeId && candidate.inputName) {
        await setOnboardingInputMapping(importPlan.draftId, {
          semanticKey: issue.field,
          fieldType: (candidate.fieldType as WorkflowFieldType | undefined) ?? "textarea",
          label: semanticFieldLabels[issue.field] ?? issue.field,
          required: true,
          targetNode: candidate.nodeId,
          targetInput: candidate.inputName,
        });
      }
      const nextPlan = await reanalyzeWorkflowImport(importPlan.draftId);
      setImportPlan(nextPlan);
    } catch (cause) {
      setImportError(normalizeProductError(cause).message);
    } finally {
      importLock.current = false;
      setImportBusy(false);
    }
  }

  async function confirmCustomWorkflowImport() {
    if (!importPlan || importBusy || importLock.current) return;
    importLock.current = true;
    setImportBusy(true);
    setImportError(undefined);
    try {
      const trimmedName = importNameDraft.trim();
      const needsMetadataUpdate =
        (trimmedName && trimmedName !== importPlan.metadata.name) ||
        importPlan.metadata.category === "unknown" ||
        importPlan.metadata.mode === "unknown" ||
        importCategoryDraft !== importPlan.metadata.category ||
        importModeDraft !== importPlan.metadata.mode;
      if (needsMetadataUpdate) {
        await setOnboardingMetadata(importPlan.draftId, {
          name: trimmedName || importPlan.metadata.name || "自定义工作流",
          workflowVersion: importPlan.metadata.workflowVersion || "1.0.0",
          recipeVersion: importPlan.metadata.recipeVersion || "1.0.0",
          category: importCategoryDraft,
          mode: importModeDraft,
        });
      }
      const action = importPlan.existingWorkflowId && importPlan.state === "ALREADY_EXISTS"
        ? "NEW_RECIPE"
        : "NEW_WORKFLOW";
      await commitWorkflowImport({
        draftId: importPlan.draftId,
        action,
        workflowId: action === "NEW_RECIPE" ? importPlan.existingWorkflowId ?? undefined : undefined,
        setCurrent: true,
      });
      setImportPlan(undefined);
      setImportNameDraft("");
      notifyBindingsChanged(projectId);
      await refresh(generation.current, true);
      setNotice("自定义 ComfyUI 工作流已导入并启用，可在上方生成器下拉框中选用。");
      window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "自定义工作流已导入并启用", tone: "success" } }));
    } catch (cause) {
      setImportError(normalizeProductError(cause).message);
    } finally {
      importLock.current = false;
      setImportBusy(false);
    }
  }

  async function cancelCustomWorkflowImport() {
    if (importPlan) {
      await discardOnboarding(importPlan.draftId).catch(() => { /* ignore */ });
    }
    setImportPlan(undefined);
    setImportNameDraft("");
    setImportError(undefined);
  }

  async function toggleCustomWorkflow(workflowVersionId: string, enabled: boolean) {
    try {
      await setWorkflowEnabled(workflowVersionId, enabled);
      notifyBindingsChanged(projectId);
      await refresh(generation.current, true);
      setNotice(enabled ? "已启用自定义工作流。" : "已停用自定义工作流。");
    } catch (cause) {
      setImportError(normalizeProductError(cause).message);
    }
  }

  async function confirmRemoveCustomWorkflow() {
    if (!removeTarget) return;
    const target = removeTarget;
    setRemoveTarget(undefined);
    try {
      await removeWorkflow(target.workflowId);
      notifyBindingsChanged(projectId);
      await refresh(generation.current, true);
      setNotice(`已移除自定义工作流「${target.name}」。历史记录与已有视频不受影响。`);
    } catch (cause) {
      setImportError(normalizeProductError(cause).message);
    }
  }

  return <section className={`generator-settings${dragOverJson ? " is-drag-over" : ""}`} aria-label="项目生成器设置" aria-busy={loading || busy || importBusy}>
    <header className="generator-settings-head"><div><h1>生成器</h1><p>设置视频和视频模式的默认生成器。只影响后续选择，不自动替换已有镜头。</p></div>{dirtySlots.length > 0 && <button type="button" className="primary" disabled={busy || !!error} onClick={() => void saveAll()}>保存全部修改（{dirtySlots.length}）</button>}</header>
    {loading && <p role="status">正在读取生成器…</p>}{(batchError || error) && <p role="alert">{[batchError, error].filter(Boolean).join(" ")}</p>}{notice && <p role="status">{notice}</p>}
    {!loading && !error && !options.length && <p role="status" data-testid="no-video-generators">暂无可用的视频生成器。请刷新设置并检查本地运行环境，或更新包含所需内置生成器的软件版本。</p>}
    {!loading && <div className="generator-settings-grid">{slots.map(slot => {
      const key = `${slot.stage}:${slot.mode}`;
      const binding = bindings.find(b => b.stage === slot.stage && b.mode === slot.mode);
      const available = options.filter(o => o.mediaKind === (slot.stage === "IMAGE" ? "image" : "video") && (slot.mode === "DEFAULT" || bindingModes(o).includes(slot.mode)));
      const selected = choices[key] ?? binding?.selectionRef ?? "";
      const isDirty = choices[key] !== undefined && choices[key] !== (binding?.selectionRef ?? "") && choices[key] !== "";
      const option = available.find(o => o.selectionRef === selected);
      return <article key={key}>
        <header className="generator-card-head">
          <h2>{slot.label}{isDirty && <span className="generator-dirty-badge">未保存修改</span>}</h2>
          {option && <span className={`generator-status-pill ${option.availability ? "ready" : "blocked"}`}>{option.availability ? "可用" : "不可用"}</span>}
        </header>
        <label><span>生成器</span><select aria-label={slot.label} disabled={busy || !!error} value={selected} onChange={e => setChoices(c => ({ ...c, [key]: e.target.value }))}>
          <option value="">未配置</option>{binding && !available.some(o => o.selectionRef === binding.selectionRef) && <option value={binding.selectionRef}>原生成器不可用，请检查本地运行环境或明确选择其他生成器</option>}
          {available.map((o, i) => <option key={o.selectionRef} value={o.selectionRef}>{generatorLabel(o, i)}{!o.availability ? " · 不可用" : ""}</option>)}
        </select></label>
        {option && <p className="generator-card-meta">输入要求：{option.fields.filter(f => "required" in f && f.required).map(fieldLabel).join("、") || "无必填素材"}{!option.availability ? " · 当前不可用，请检查本地节点、模型与运行环境" : ""}</p>}
        <div className="generator-card-foot"><button type="button" className={isDirty ? "primary" : undefined} disabled={busy || !!error || !option?.availability || selected === binding?.selectionRef} onClick={() => void save(slot.stage, slot.mode, selected)}>保存设置</button></div>
      </article>;
    })}</div>}
    <section className={`custom-workflow-panel${dragOverJson ? " is-drag-over" : ""}`} aria-label="自定义 ComfyUI 工作流">
      <div className="custom-workflow-head">
        <div>
          <h2>自定义 ComfyUI 工作流</h2>
          <p>支持直接导入 ComfyUI 导出的 API 格式或普通保存的 UI 格式 JSON（自动连接本机 ComfyUI 校验节点与槽位），也可直接将 .json 文件拖入此页面。</p>
        </div>
        <button type="button" className="primary" disabled={importBusy || Boolean(importPlan)} onClick={() => void startImportFromPicker()}>
          {importBusy ? "正在识别工作流…" : "导入自定义工作流（JSON）"}
        </button>
      </div>
      {importError && <p role="alert" className="custom-workflow-error">{importError}</p>}
      {importPlan && (
        <article className="custom-workflow-review" aria-label="工作流识别预览">
          <header className="generator-card-head">
            <h3>识别结果预览</h3>
            <span className={`generator-status-pill ${importPlan.issues.length === 0 ? "ready" : "blocked"}`}>
              {friendlyCategory(importPlan.metadata.category)} · {friendlyMode(importPlan.metadata.mode)}
            </span>
          </header>
          <p className="generator-card-meta">{importPlan.message}</p>
          <div className="custom-workflow-meta-row">
            <label>
              <span>工作流名称</span>
              <input
                type="text"
                value={importNameDraft}
                maxLength={80}
                disabled={importBusy}
                onChange={(e) => setImportNameDraft(e.target.value)}
              />
            </label>
            <label>
              <span>类型</span>
              <select value={importCategoryDraft} disabled={importBusy} onChange={(e) => setImportCategoryDraft(e.target.value)}>
                <option value="video">视频</option>
                <option value="image">图片</option>
              </select>
            </label>
            <label>
              <span>模式</span>
              <select value={importModeDraft} disabled={importBusy} onChange={(e) => setImportModeDraft(e.target.value)}>
                <option value="image_to_video">图生视频</option>
                <option value="text_to_video">文生视频</option>
                <option value="first_last_frame">首尾帧视频</option>
                <option value="reference_video">参考图视频</option>
                <option value="text_to_image">文生图</option>
                <option value="image_to_image">图生图</option>
              </select>
            </label>
          </div>
          {importPlan.capability?.issues?.length > 0 && (
            <div className="custom-workflow-missing-nodes" role="status">
              <strong>运行环境检查提示：</strong>
              <ul>
                {importPlan.capability.issues.map((issue, idx) => (
                  <li key={`${issue.code}-${idx}`}>
                    {issue.classType ? `缺少自定义节点「${issue.classType}」` : issue.message}
                    {issue.classType && issue.message ? ` — ${issue.message}` : ""}
                  </li>
                ))}
              </ul>
            </div>
          )}
          {importPlan.inferences.length > 0 && (
            <div className="custom-workflow-tags">
              <strong>自动识别参数：</strong>
              {importPlan.inferences.map((item, idx) => (
                <span key={`${item.field}-${idx}`} className="custom-workflow-tag">
                  {semanticFieldLabels[item.field] ?? item.field}
                </span>
              ))}
            </div>
          )}
          {importPlan.issues.length > 0 && (
            <div className="custom-workflow-issues">
              {importPlan.issues.map((issue, idx) => (
                <div key={`${issue.code}-${idx}`} className="custom-workflow-issue-item">
                  <p>{issue.message}</p>
                  {issue.candidates.length > 0 && (
                    <div className="custom-workflow-candidates">
                      {issue.candidates.map((cand, cIdx) => (
                        <button
                          key={`${cand.label}-${cIdx}`}
                          type="button"
                          disabled={importBusy}
                          onClick={() => void resolveImportCandidate(issue, cand)}
                        >
                          选择：{cand.label}
                        </button>
                      ))}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
          <div className="generator-card-foot">
            {importPlan.normalizationState === "UI_SOURCE_PENDING" ? (
              <button type="button" className="primary" disabled={importBusy} onClick={() => void retryImportAnalysis()}>
                {importBusy ? "正在重试识别…" : "重试识别（需 ComfyUI 在线）"}
              </button>
            ) : importPlan.state === "ALREADY_EXISTS_ARCHIVED" && importPlan.existingWorkflowId ? (
              <button type="button" className="primary" disabled={importBusy} onClick={() => void restoreArchivedWorkflow()}>
                {importBusy ? "正在恢复…" : "一键恢复已有工作流"}
              </button>
            ) : (
              <button
                type="button"
                className="primary"
                disabled={importBusy}
                onClick={() => void confirmCustomWorkflowImport()}
              >
                {importBusy ? "正在保存…" : "确认导入并启用"}
              </button>
            )}
            <button type="button" disabled={importBusy} onClick={() => void cancelCustomWorkflowImport()}>取消</button>
          </div>
        </article>
      )}
      {customWorkflows.length > 0 && (
        <div className="custom-workflow-list">
          <h3>已导入的自定义工作流（{customWorkflows.length}）</h3>
          {customWorkflows.map((item) => {
            const versionId = item.registry.currentVersionId;
            const rt = currentWorkflowRuntime(item);
            const enabled = rt?.enabled ?? false;
            return (
              <div key={item.registry.workflowId} className="custom-workflow-row">
                <div>
                  <strong>{item.registry.name}</strong>
                  <small>
                    {friendlyCategory(rt?.category)} · {friendlyMode(rt?.mode)} · {rt ? enabled ? "已启用" : "已停用" : "状态待核对"}
                    {rt?.readiness ? ` · ${rt.readiness === "READY" ? "当前环境可用" : "需检查本地节点/模型"}` : ""}
                  </small>
                </div>
                <div className="custom-workflow-row-actions">
                  {versionId && rt && (
                    <button type="button" onClick={() => void toggleCustomWorkflow(versionId, !enabled)}>
                      {enabled ? "停用" : "启用"}
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => setRemoveTarget({ workflowId: item.registry.workflowId, name: item.registry.name })}
                  >
                    移除
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </section>
    <ThemedConfirmDialog
      open={Boolean(removeTarget)}
      title="移除自定义工作流"
      message={removeTarget ? `确定移除「${removeTarget.name}」吗？已有镜头绑定和历史生成视频不会被删除。` : ""}
      confirmLabel="确认移除"
      danger
      onConfirm={() => void confirmRemoveCustomWorkflow()}
      onCancel={() => setRemoveTarget(undefined)}
    />
    <footer className="generator-settings-footer">
      <button type="button" disabled={loading || busy} onClick={() => void handleRefresh()}>刷新设置</button>
      <button type="button" onClick={() => navigate({ kind: "system-settings", section: "general", returnTo: { kind: "project-settings", projectId, section: "generators" } })}>检查本地运行环境</button>
    </footer>
  </section>;
}
