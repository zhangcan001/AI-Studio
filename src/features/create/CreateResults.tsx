import { useEffect, useState } from "react";
import { productClient } from "../../product/client";
import { normalizeProductError } from "../../product/errors";
import type { CreationAsset } from "../../product/types";
import type { CreateController } from "./CreateController";
import { modeLabel } from "../../product/generatorPresentation";
import { candidateForStage, runLabels, type CreateRoute } from "./createModel";
import { focusCreateInput, resolveCreateReadinessAction, type CreateReadinessAction } from "./createReadinessAction";
export function RunStatusCard({ controller: c }: { controller: CreateController }) {
  const edit = () => document.getElementById("create-field-prompt")?.focus();
  return <section aria-label="本次运行" aria-live="polite"><h2>本次运行</h2>
    {c.accepted && <p>{c.accepted.startOutcome === "FAILED_TO_START" ? "已保存运行，启动失败" : c.accepted.startOutcome === "QUEUED" ? "任务已排队，将在当前任务结束后自动启动" : "请求已接受"}</p>}
    {c.run && <><p className="create-state-pill">{runLabels[c.run.status]}</p><p>{c.run.progress.succeeded}/{c.run.progress.total} 已完成</p><p>{c.run.progress.failed} 失败 · {c.run.progress.cancelled} 已取消</p>{c.run.errorSummary && <p>运行遇到问题，请查看详情。</p>}</>}
    {!c.runRef && <p>尚未生成</p>}
    {c.run?.availableActions.includes("EDIT_INPUT") && <button type="button" onClick={edit}>修改输入</button>}
    {c.run?.availableActions.includes("RETRY") && <button type="button" disabled={c.busy} onClick={() => void c.retry()}>重试原运行</button>}
    {c.runRef && <button type="button" onClick={c.openRun}>查看运行详情</button>}
    <small>重试使用原运行快照；修改草稿后点击生成会创建新的运行。</small>
  </section>;
}
function AssetPreview({ asset, projectId }: { asset: CreationAsset; projectId: string }) {
  const [thumbnail, setThumbnail] = useState<string>();
  useEffect(() => {
    if (!asset.thumbnailBytes?.length) { setThumbnail(undefined); return; }
    const url = URL.createObjectURL(new Blob([new Uint8Array(asset.thumbnailBytes)], { type: "image/png" }));
    setThumbnail(url); return () => URL.revokeObjectURL(url);
  }, [asset.thumbnailBytes]);
  if (asset.mediaKind === "video") return <video aria-label={asset.name} controls preload="metadata" src={productClient.creation.mediaUrl(projectId, asset.id, "video")} />;
  return thumbnail ? <img src={thumbnail} alt={asset.name} /> : <div className="create-preview-placeholder">{asset.name}</div>;
}
export function CandidatePanel({ controller: c, route }: { controller: CreateController; route: CreateRoute }) {
  const candidates = c.context?.candidates.filter(asset => candidateForStage(asset, route)) ?? [];
  return <section aria-label="当前镜头候选"><h2>当前镜头候选</h2><button type="button" disabled={c.busy} onClick={() => void c.refresh()}>刷新结果</button>
    {c.run?.status === "FAILED" && <p>本次生成失败；已有候选和选用状态不会删除。</p>}
    {!candidates.length && <p>尚无此阶段的生成候选。</p>}<div className="create-candidates">{candidates.map(asset => <article key={asset.id} className={asset.selected ? "selected" : ""}>
      <AssetPreview asset={asset} projectId={route.projectId} /><p>{asset.name}</p><strong>{asset.selected ? "已选用" : "待选择"}</strong><button type="button" disabled={c.busy || asset.selected} onClick={() => void c.selectResult(asset.id)}>选用此结果</button>
    </article>)}</div><small>选用不等于审核；其他候选与历史运行会保留。</small></section>;
}
/** Readiness + errors for the composer; the composer's submit button is the only generate trigger. */
export function ReadinessStatus({ controller: c }: { controller: CreateController }) {
  const issues = c.readiness?.issues ?? [];
  function act(action: CreateReadinessAction) {
    switch (action.kind) {
      case "focus-field": focusCreateInput(action.field); break;
      case "focus-inputs": focusCreateInput(); break;
      case "select-generator": focusCreateInput("selectionRef"); break;
      case "open-runtime-settings": c.openRuntimeSettings(); break;
      case "recheck": void c.recheckReadiness(); break;
      case "open-projects": c.openProjects(); break;
      case "none": break;
    }
  }
  const readyCopy = c.readinessState === "RUNTIME_OFFLINE" ? "运行环境暂时不可用" : c.readinessState === "READY" ? "可以入队生成" : c.readinessState === "NOT_READY" ? "请检查输入或运行环境" : "正在检查准备状态";
  const num = (key: string) => { const v = c.values[key]; return v?.type === "integer" || v?.type === "number" ? v.value : undefined; };
  const summary = [num("duration_seconds") && `${num("duration_seconds")} 秒`, num("width") && num("height") && `${num("width")} × ${num("height")}`, c.generator && modeLabel(c.generator)].filter(Boolean).join(" · ");
  return <div className="composer-status" data-state={c.readinessState} aria-live="polite"><strong className="create-ready-state">{readyCopy}</strong>{summary && <span className="create-ready-summary">已设置 {summary}</span>}
    {issues.map((issue, index) => {
      // Map known product codes to catalog copy; never collapse typed readiness codes to INTERNAL_ERROR.
      const error = normalizeProductError(issue);
      const action = resolveCreateReadinessAction(error.details);
      return <p key={`${error.code}-${index}`}>{error.message}{action.kind === "none" ? <small>{action.explanation}</small> : <button type="button" disabled={c.busy} onClick={() => act(action)}>{action.label}</button>}</p>;
    })}
    {c.error && <p role="alert">{c.error}</p>}
    {c.videoInputs.enabled && (c.videoInputs.loading || c.videoInputs.busy) && <small>{c.videoInputs.loading ? "正在读取输入…" : "正在保存输入…"}</small>}
    {c.videoInputs.importReceipt && <p role="status">{c.videoInputs.importReceipt}</p>}{c.videoInputs.error && <p role="alert">{c.videoInputs.error}</p>}
  </div>;
}
