import { useEffect, useRef, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { GeneratorBindingSummary, GeneratorOption } from "../../product/types";
import type { AppRoute } from "../../app/routes/types";
import { generatorLabel, fieldLabel } from "../../product/generatorPresentation";
import "./GeneratorSettingsPage.css";

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

export function GeneratorSettingsPage({ projectId, navigate }: { projectId: string; navigate: (route: AppRoute) => unknown }) {
  const [options, setOptions] = useState<GeneratorOption[]>([]);
  const [bindings, setBindings] = useState<GeneratorBindingSummary[]>([]);
  const [choices, setChoices] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();
  const [notice, setNotice] = useState<string>();
  const generation = useRef(0);
  const locked = useRef(false);
  async function refresh(token: number) {
    const [overview, images, videos] = await Promise.all([
      productClient.project.getOverview(projectId), productClient.creation.generatorsList(projectId, "image"), productClient.creation.generatorsList(projectId, "video"),
    ]);
    if (token !== generation.current) return;
    // No recommendation/name inference is a substitute for a persisted binding.
    if (!overview.generatorBindings) throw new Error("暂时无法读取生成器设置，请刷新。");
    setBindings(overview.generatorBindings); setOptions([...images, ...videos]); setChoices({});
  }
  useEffect(() => {
    const token = ++generation.current;
    setLoading(true); setError(undefined); setNotice(undefined); setOptions([]); setBindings([]);
    void refresh(token).catch(e => { if (token === generation.current) setError(normalizeProductError(e).message); }).finally(() => { if (token === generation.current) setLoading(false); });
    return () => { generation.current++; };
  }, [projectId]);
  const slots = [{ stage: "IMAGE", mode: "DEFAULT", label: "默认图片生成器" }, { stage: "VIDEO", mode: "DEFAULT", label: "默认视频生成器" },
    ...Array.from(new Set([...options.filter(o => o.mediaKind === "video").flatMap(bindingModes), ...bindings.filter(b => b.stage === "VIDEO").map(b => b.mode)]))
      .filter(mode => mode !== "DEFAULT").map(mode => ({ stage: "VIDEO", mode, label: `${bindingModeLabels[mode] ?? "历史视频模式"}生成器` }))];
  async function save(stage: string, mode: string, selectionRef: string) {
    if (locked.current || !selectionRef) return;
    locked.current = true; setBusy(true); setError(undefined); setNotice(undefined);
    const token = generation.current;
    const current = bindings.find(b => b.stage === stage && b.mode === mode);
    try {
      await productClient.project.generatorBindingSet(projectId, { stage, mode, selectionRef, expectedRevision: current?.revision ?? null, expectedBindingInstanceId: current?.bindingInstanceId ?? null });
      if (token === generation.current) { await refresh(token); setNotice("生成器设置已保存；已有镜头与历史运行不变。"); }
    } catch (e) {
      if (token !== generation.current) return;
      const failure = normalizeProductError(e);
      setError(failure.message);
      // Refresh after a conflict, but never retry or silently overwrite it.
      try { await refresh(token); } catch { setNotice("刷新失败，请重新打开生成器设置。尚未重试保存。"); }
    } finally { locked.current = false; if (token === generation.current) setBusy(false); }
  }
  return <section className="generator-settings" aria-label="项目生成器设置" aria-busy={loading || busy}>
    <header><h1>生成器</h1><p>设置图片、视频和视频模式的默认生成器。只影响后续选择，不自动替换已有镜头。</p></header>
    {loading && <p role="status">正在读取生成器…</p>}{error && <p role="alert">{error}</p>}{notice && <p role="status">{notice}</p>}
    {!loading && slots.map(slot => {
      const key = `${slot.stage}:${slot.mode}`;
      const binding = bindings.find(b => b.stage === slot.stage && b.mode === slot.mode);
      const available = options.filter(o => o.mediaKind === (slot.stage === "IMAGE" ? "image" : "video") && (slot.mode === "DEFAULT" || bindingModes(o).includes(slot.mode)));
      const selected = choices[key] ?? binding?.selectionRef ?? "";
      const option = available.find(o => o.selectionRef === selected);
      return <article key={key}><h2>{slot.label}</h2><label>生成器<select aria-label={slot.label} disabled={busy || !!error} value={selected} onChange={e => setChoices(c => ({ ...c, [key]: e.target.value }))}>
        <option value="">未配置</option>{binding && !available.some(o => o.selectionRef === binding.selectionRef) && <option value={binding.selectionRef}>原生成器不可用，请到高级工作流检查</option>}
        {available.map((o, i) => <option key={o.selectionRef} value={o.selectionRef}>{generatorLabel(o, i)}{!o.availability ? " · 不可用" : ""}</option>)}
      </select></label>
      {option && <><p>{generatorLabel(option, available.indexOf(option))}</p><p>输入要求：{option.fields.filter(f => "required" in f && f.required).map(fieldLabel).join("、") || "无必填素材"}</p><p>{option.availability ? "可用" : "当前不可用，请在高级工作流中检查"}</p></>}
      <button type="button" disabled={busy || !!error || !option?.availability || selected === binding?.selectionRef} onClick={() => void save(slot.stage, slot.mode, selected)}>保存设置</button>
    </article>; })}
    <button type="button" disabled={busy} onClick={() => void refresh(generation.current).then(() => setError(undefined)).catch(e => setError(normalizeProductError(e).message))}>刷新设置</button>
    <details><summary>高级</summary><p>管理工作流、映射、版本和高级验证，不会自动修改项目生成器。</p><button type="button" onClick={() => navigate({ kind: "system-settings", section: "advanced-workflows", returnTo: { kind: "project-settings", projectId, section: "generators" } })}>打开高级工作流 Lab</button></details>
  </section>;
}
