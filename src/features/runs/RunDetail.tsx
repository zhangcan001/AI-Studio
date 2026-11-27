import { useEffect, useState } from "react";
import type { RunsProps, RunsController } from "./RunsController";
import { actionLabel, friendlyGenerator, inputText, runStatusLabel, sourceLabels } from "./runsModel";
import { RunResults } from "./RunResults";
import { runRecoveryPresentation } from "./runRecoveryPresentation";
import { useTaskStore } from "../../stores/taskStore";

function formatRunElapsed(startedAt?: string, nowMs = Date.now()): string | undefined {
  if (!startedAt) return undefined;
  const start = Date.parse(startedAt);
  if (!Number.isFinite(start)) return undefined;
  const sec = Math.max(0, Math.floor((nowMs - start) / 1000));
  return `${String(Math.floor(sec / 60)).padStart(2, "0")}:${String(sec % 60).padStart(2, "0")}`;
}

function copyPromptText(text: string) {
  void navigator.clipboard?.writeText(text).then(() => {
    window.dispatchEvent(new CustomEvent("ai-studio:toast", { detail: { message: "已复制提示词", tone: "info" } }));
  }).catch(() => {});
}

export function RunDetail(props: RunsProps & { controller: RunsController }) {
  const c = props.controller;
  const run = c.detail;
  const recentTasks = useTaskStore(state => state.recentTasks);
  const [nowTick, setNowTick] = useState(() => Date.now());
  const isRunning = run?.status === "RUNNING";
  const liveTask = isRunning
    ? (run.ref.source === "task"
      ? recentTasks.find(t => t.id === run.ref.id)
      : recentTasks.find(t => run.detail?.inputs.some(i => i.taskId === t.id) && ["RUNNING", "COLLECTING", "PREPARING"].includes(t.status))
        ?? recentTasks.find(t => t.projectId === run.projectId && ["RUNNING", "COLLECTING", "PREPARING"].includes(t.status)))
    : undefined;
  useEffect(() => {
    if (!isRunning) return;
    setNowTick(Date.now());
    const id = window.setInterval(() => setNowTick(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [isRunning, liveTask?.id]);

  if (!run) {
    if (c.detailLoading && props.route.run) {
      return (
        <section className="run-detail run-detail-empty" aria-label="运行详情">
          <p role="status">正在读取运行详情…</p>
        </section>
      );
    }
    const items = c.list?.items ?? [];
    const activeCount = items.filter(item => ["QUEUED", "RUNNING", "PAUSED"].includes(item.status)).length;
    const failedCount = items.filter(item => item.status === "FAILED" || item.status === "PARTIAL").length;
    return (
      <section className="run-detail run-detail-empty">
        <div className="run-empty-card">
          <strong>{c.missing ? "运行已不存在或不可访问。请从列表选择其他运行。" : "选择一个运行查看详情。"}</strong>
          {!c.missing && items.length > 0 && (
            <>
              <div className="run-empty-stats">
                <span>近期共 {items.length} 条</span>
                {activeCount > 0 && <span className="status-running">进行中 {activeCount}</span>}
                {failedCount > 0 && <span className="status-failed">需处理 {failedCount}</span>}
              </div>
              <button type="button" className="primary" onClick={() => props.navigate({ ...props.route, run: items[0].ref })}>
                打开最近一条运行
              </button>
              <small>可使用键盘 ↑ / ↓ 方向键快速切换运行记录</small>
            </>
          )}
        </div>
      </section>
    );
  }

  const recovery = runRecoveryPresentation(run);
  const finished = run.progress.succeeded + run.progress.failed + run.progress.cancelled;
  const canDelete = run.ref.source !== "production-run" && ["SUCCEEDED", "FAILED", "PARTIAL", "CANCELLED"].includes(run.status);
  const liveStep = liveTask?.progress.mode === "step" && liveTask.progress.total && liveTask.progress.total > 0
    ? {
      current: liveTask.progress.current ?? 0,
      total: liveTask.progress.total,
      pct: Math.min(100, Math.round(((liveTask.progress.current ?? 0) / liveTask.progress.total) * 100)),
    }
    : undefined;
  const liveElapsed = isRunning ? formatRunElapsed(liveTask?.startedAt ?? liveTask?.createdAt ?? run.updatedAt, nowTick) : undefined;

  return (
    <section className="run-detail" aria-label="运行详情">
      <div className="run-detail-hero">
        <header>
          <div className="run-detail-title-group">
            <h2>{run.title}</h2>
            <span className="run-source">{sourceLabels[run.ref.source]}</span>
            <span className={`run-status status-${run.status.toLowerCase()}`}>{runStatusLabel(run)}</span>
          </div>
          <div className="run-actions">
            {(["START", "PAUSE", "CANCEL"] as const)
              .filter(action => run.availableActions.includes(action))
              .map(action => (
                <button
                  type="button"
                  className={action === "START" ? "primary" : action === "CANCEL" ? "danger" : undefined}
                  disabled={c.busy}
                  key={action}
                  onClick={() => void c.action(action)}
                >
                  {actionLabel(action, run.status)}
                </button>
              ))}
            {canDelete && (
              <button type="button" className="danger" disabled={c.busy} onClick={() => void c.remove()}>
                删除记录
              </button>
            )}
          </div>
        </header>

        <div className="run-progress-strip">
          <div className="run-progress-meta">
            <span>{run.progress.succeeded}/{run.progress.total} 已完成 · {run.progress.failed} 项失败 · {run.progress.cancelled} 项取消</span>
            <details className="run-quiet-help">
              <summary>ⓘ 运行控制与复用说明</summary>
              <p role="note">启动需显式确认；暂停不抢占正在推理的项；取消会中断正在生成的任务并停止后续镜头，已生成的视频和素材会保留。</p>
              {(recovery.resultsPreserved || c.results.length > 0) && <p>已完成的结果会保留。重试失败项或编辑输入创建新运行，不会删除当前运行已经产生的结果。</p>}
              <p>点击“编辑这些输入并创建新运行”会打开创作页并载入历史输入。只有再次确认生成后，才会创建新的运行；不会覆盖原运行。</p>
            </details>
          </div>
          <progress max={Math.max(1, run.progress.total)} value={finished} />
          {isRunning && (liveStep || liveElapsed) && (
            <div className="run-live-sampler" aria-label="实时采样进度">
              <div className="run-live-sampler-head">
                <span>{liveStep ? `ComfyUI 实时采样 · 第 ${liveStep.current}/${liveStep.total} 步（${liveStep.pct}%）` : "ComfyUI 节点执行中…"}</span>
                {liveElapsed && <span>已耗时 {liveElapsed}</span>}
              </div>
              {liveStep && <progress max={liveStep.total} value={liveStep.current} />}
            </div>
          )}
        </div>
      </div>

      {recovery.showRecovery && (
        <section className="run-recovery-card" role="status" aria-label="运行恢复说明">
          <h3>{recovery.heading}</h3>
          <p>{recovery.summary}</p>
          {run.errorSummary && run.errorSummary !== recovery.summary && <p className="run-recovery-error">{run.errorSummary}</p>}
          {recovery.retry && (
            <>
              <p className="run-recovery-count">可恢复失败项：{recovery.retry.count} 项</p>
              <p>{recovery.retry.explanation}</p>
              <div className="run-inline-actions">
                {run.ref.source !== "production-run" && (
                  <button type="button" className="primary" disabled={c.busy} onClick={() => void c.retryAndStart()}>
                    重试并立即启动
                  </button>
                )}
                <button type="button" disabled={c.busy} onClick={() => void c.action("RETRY")}>
                  {recovery.retry.label}
                </button>
              </div>
            </>
          )}
          {recovery.needsInputEdit && <p>{recovery.inputEditExplanation}</p>}
        </section>
      )}

      <RunResults {...props} />

      <section className="run-sources-section">
        <h3>关联来源</h3>
        {run.detail?.sources.length ? (
          <div className="run-sources-list">
            {run.detail.sources.map(source => (
              <button
                type="button"
                className="run-source-chip"
                key={`${source.id}:${source.stage}`}
                onClick={() => props.navigate({ kind: "create", projectId: props.route.projectId, shotId: source.id, stage: source.stage === "video" ? "video" : "image" })}
              >
                {source.name} · {source.stage === "video" ? "视频" : "图片"}
              </button>
            ))}
          </div>
        ) : (
          <p className="runs-empty-note">历史独立运行，没有关联镜头。</p>
        )}
      </section>

      <section className="run-inputs-section">
        <h3>输入摘要{run.detail?.inputs.length ? `（${run.detail.inputs.length}）` : ""}</h3>
        <div className="run-inputs-list">
          {run.detail?.inputs.map((input, index) => {
            const entries = inputText(input);
            const promptEntries = entries.filter(item => item.key === "prompt" || item.key === "negative_prompt");
            const paramEntries = entries.filter(item => item.key !== "prompt" && item.key !== "negative_prompt");
            const canRetryItem = run.ref.source === "queue-batch" && input.itemId && run.availableActions.includes("RETRY") && run.recoverability.retryItemIds.includes(input.itemId);
            return (
              <article className={`run-input${input.errorMessage ? " run-input-failed" : ""}`} key={input.taskId ?? input.itemId ?? index}>
                <div className="run-input-head">
                  <h4>{input.shotName ? `${input.shotName} · ` : ""}{friendlyGenerator(input.generatorName)} · 输入 {index + 1}</h4>
                  {input.executionStatus && <span className="run-source">{input.executionStatus}</span>}
                </div>
                {promptEntries.map(item => (
                  <div className="run-input-prompt" key={item.key}>
                    <div className="run-input-prompt-head">
                      <strong>{item.label}</strong>
                      <button type="button" className="run-copy-btn" onClick={() => copyPromptText(item.value)}>复制</button>
                    </div>
                    <p>{item.value}</p>
                  </div>
                ))}
                {paramEntries.length > 0 && (
                  <div className="run-param-chips">
                    {paramEntries.map(item => (
                      <span className="run-param-chip" key={item.key}>
                        <strong>{item.label}：</strong>{item.value}
                      </span>
                    ))}
                  </div>
                )}
                {input.errorMessage && <p className="run-input-error">失败原因：{input.errorMessage}</p>}
                <div className="run-inline-actions">
                  {input.shotId && <button type="button" disabled={c.busy} onClick={() => void c.openShot(input)}>编辑此镜头</button>}
                  {canRetryItem && (
                    <>
                      <button type="button" className="primary" disabled={c.busy} title="立即恢复整个队列批次（含批次内其他待执行项），已完成结果保持不变" onClick={() => void c.retryInputAndStart(input)}>重试此镜头并启动批次</button>
                      <button type="button" disabled={c.busy} onClick={() => void c.retryInput(input)}>仅重试此镜头</button>
                    </>
                  )}
                  <button type="button" disabled={c.busy || !input.selectionRef} title="打开创作页并载入历史输入，不覆盖原运行" onClick={() => void c.reuse(input)}>编辑这些输入并创建新运行</button>
                </div>
                {input.reuseUnavailableReason && <p className="run-input-unavailable">{input.reuseUnavailableReason}</p>}
              </article>
            );
          })}
        </div>
        {!run.detail?.inputs.length && <p className="runs-empty-note">尚未创建任务；启动前准入会重新检查输入。</p>}
      </section>

      <details className="run-tech-details">
        <summary>高级技术详情</summary>
        <div className="run-inline-actions">
          {[...new Set([...(run.ref.source === "task" ? [run.ref.id] : []), ...(run.detail?.inputs.flatMap(input => input.taskId ? [input.taskId] : []) ?? [])])].map((id, index) => (
            <button type="button" key={id} onClick={() => props.navigate({ kind: "system-settings", section: "diagnostics", returnTo: { kind: "runs", projectId: props.route.projectId, run: { source: "task", id } } })}>
              技术诊断 · 任务 {index + 1}
            </button>
          ))}
        </div>
        <pre>{JSON.stringify({ ref: run.ref, phase: run.phase, parent: run.preferredParent, inputs: run.detail?.inputs.map(input => ({ taskId: input.taskId, selectionRef: input.selectionRef })) }, null, 2)}</pre>
      </details>
    </section>
  );
}
