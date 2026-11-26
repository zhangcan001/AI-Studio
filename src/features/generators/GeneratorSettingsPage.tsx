import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import { GENERATOR_BINDINGS_CHANGED, LAST_GENERATOR_SESSION_KEY, type GeneratorBindingSummary, type GeneratorOption } from "../../product/types";
import type { AppRoute } from "../../app/routes/types";
import { generatorLabel, fieldLabel } from "../../product/generatorPresentation";
import "./GeneratorSettingsPage.css";

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
  const generation = useRef(0);
  const readSeq = useRef(0);
  const locked = useRef(false);
  const retainedConflictKeysRef = useRef<Set<string>>(new Set());
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
    onDirtyChange?.(dirtySlots.length > 0);
    return () => onDirtyChange?.(false);
  }, [dirtySlots.length, onDirtyChange]);
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
      if (token === generation.current) setNotice("生成器设置已保存；已有镜头与历史运行不变。");
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
        if (token === generation.current) setNotice(`已批量保存 ${savedCount} 项生成器设置。`);
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
  return <section className="generator-settings" aria-label="项目生成器设置" aria-busy={loading || busy}>
    <header className="generator-settings-head"><div><h1>生成器</h1><p>设置视频和视频模式的默认生成器。只影响后续选择，不自动替换已有镜头。</p></div>{dirtySlots.length > 0 && <button type="button" className="primary" disabled={busy || !!error} onClick={() => void saveAll()}>保存全部修改（{dirtySlots.length}）</button>}</header>
    {loading && <p role="status">正在读取生成器…</p>}{(batchError || error) && <p role="alert">{[batchError, error].filter(Boolean).join(" ")}</p>}{notice && <p role="status">{notice}</p>}
    {!loading && !error && !options.length && <p role="status" data-testid="no-video-generators">暂无可用的视频生成器。请刷新设置并检查本地运行环境，或更新包含所需内置生成器的软件版本。</p>}
    {!loading && slots.map(slot => {
      const key = `${slot.stage}:${slot.mode}`;
      const binding = bindings.find(b => b.stage === slot.stage && b.mode === slot.mode);
      const available = options.filter(o => o.mediaKind === (slot.stage === "IMAGE" ? "image" : "video") && (slot.mode === "DEFAULT" || bindingModes(o).includes(slot.mode)));
      const selected = choices[key] ?? binding?.selectionRef ?? "";
      const isDirty = choices[key] !== undefined && choices[key] !== (binding?.selectionRef ?? "") && choices[key] !== "";
      const option = available.find(o => o.selectionRef === selected);
      return <article key={key}><h2>{slot.label}{isDirty && <span className="generator-dirty-badge">未保存修改</span>}</h2><label>生成器<select aria-label={slot.label} disabled={busy || !!error} value={selected} onChange={e => setChoices(c => ({ ...c, [key]: e.target.value }))}>
        <option value="">未配置</option>{binding && !available.some(o => o.selectionRef === binding.selectionRef) && <option value={binding.selectionRef}>原生成器不可用，请检查本地运行环境或明确选择其他生成器</option>}
        {available.map((o, i) => <option key={o.selectionRef} value={o.selectionRef}>{generatorLabel(o, i)}{!o.availability ? " · 不可用" : ""}</option>)}
      </select></label>
      {option && <><p>{generatorLabel(option, available.indexOf(option))}</p><p>输入要求：{option.fields.filter(f => "required" in f && f.required).map(fieldLabel).join("、") || "无必填素材"}</p><p>{option.availability ? "可用" : "当前不可用，请检查本地节点、模型与运行环境"}</p></>}
      <button type="button" disabled={busy || !!error || !option?.availability || selected === binding?.selectionRef} onClick={() => void save(slot.stage, slot.mode, selected)}>保存设置</button>
    </article>; })}
    <button type="button" disabled={loading || busy} onClick={() => void handleRefresh()}>刷新设置</button>
    <button type="button" onClick={() => navigate({ kind: "system-settings", section: "general", returnTo: { kind: "project-settings", projectId, section: "generators" } })}>检查本地运行环境</button>
  </section>;
}
