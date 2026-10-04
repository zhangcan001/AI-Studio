import { useCallback, useEffect, useRef, useState } from "react";
import { diagnosticsClient } from "../../services/diagnosticsClient";
import type { DiagnosticExecutionHealth, DiagnosticRecentFailure, DiagnosticTaskTimeline, DiagnosticDurationSample } from "../../types/diagnostics";
import { formatDateTime } from "../../i18n/statusLabels";
import { toUserMessage } from "../../i18n/errorMessages";

export interface DiagnosticsExecutionProps {
  projectId?: string;
  initialTaskId?: string;
  onOpenRun?: (projectId: string, taskId: string) => void;
  onOpenAudit?: (projectId: string, taskId: string) => void;
}

/** Component-owned read lifetime only: no polling, subscription, cache or domain state. */
export function DiagnosticsExecutionPanel(props: DiagnosticsExecutionProps) {
  const { projectId, initialTaskId } = props;
  const [health, setHealth] = useState<DiagnosticExecutionHealth>();
  const [failures, setFailures] = useState<DiagnosticRecentFailure[]>([]);
  const [timeline, setTimeline] = useState<DiagnosticTaskTimeline>();
  const [taskId, setTaskId] = useState(initialTaskId ?? "");
  const [loading, setLoading] = useState(false);
  const [timelineLoading, setTimelineLoading] = useState(false);
  const [error, setError] = useState<string>();
  const [timelineError, setTimelineError] = useState<string>();
  const lifetime = useRef(0);
  const refreshOwner = useRef<number | null>(null);
  const timelineRequest = useRef(0);

  const refresh = useCallback(async () => {
    if (!projectId || refreshOwner.current === lifetime.current) return;
    const owner = lifetime.current;
    refreshOwner.current = owner;
    setLoading(true); setError(undefined); setHealth(undefined); setFailures([]);
    try {
      const [nextHealth, nextFailures] = await Promise.all([
        diagnosticsClient.executionHealth(projectId), diagnosticsClient.recentFailures(projectId),
      ]);
      if (owner !== lifetime.current) return;
      if (nextHealth.projectId !== projectId || nextFailures.some(item => item.projectId !== projectId)) {
        throw new Error("诊断响应项目不一致，请重新打开当前项目。");
      }
      setHealth(nextHealth); setFailures(nextFailures);
    } catch (nextError) {
      if (owner === lifetime.current) setError(toUserMessage(nextError));
    } finally {
      if (owner === lifetime.current) { refreshOwner.current = null; setLoading(false); }
    }
  }, [projectId]);

  const openTimeline = useCallback(async (id: string) => {
    if (!projectId || !id.trim()) return;
    const owner = lifetime.current, request = ++timelineRequest.current;
    setTaskId(id); setTimeline(undefined); setTimelineError(undefined); setTimelineLoading(true);
    try {
      const next = await diagnosticsClient.taskTimeline(projectId, id);
      if (owner !== lifetime.current || request !== timelineRequest.current) return;
      if (next.projectId !== projectId || next.taskId !== id) throw new Error("任务诊断定位不一致。");
      setTimeline(next);
    } catch (nextError) {
      if (owner === lifetime.current && request === timelineRequest.current) setTimelineError(toUserMessage(nextError));
    } finally {
      if (owner === lifetime.current && request === timelineRequest.current) setTimelineLoading(false);
    }
  }, [projectId]);

  useEffect(() => {
    lifetime.current += 1; refreshOwner.current = null; timelineRequest.current += 1;
    setHealth(undefined); setFailures([]); setTimeline(undefined); setTimelineError(undefined);
    setTaskId(initialTaskId ?? ""); setTimelineLoading(false);
    void refresh();
    if (initialTaskId) void openTimeline(initialTaskId);
    return () => { lifetime.current += 1; refreshOwner.current = null; timelineRequest.current += 1; };
  }, [projectId, initialTaskId, refresh, openTimeline]);

  return <section className="settings-card diagnostics-execution" aria-labelledby="diagnostic-execution-title">
    <div className="settings-card-heading"><h3 id="diagnostic-execution-title">执行健康与技术诊断</h3>
      <button type="button" disabled={!projectId || loading || timelineLoading} onClick={() => { void refresh(); if (taskId) void openTimeline(taskId); }}>刷新执行诊断</button></div>
    {!projectId && <p>未选择项目；系统健康仍可独立查看。</p>}
    {loading && <p role="status">正在读取最近任务…</p>}
    {error && <p role="alert">{error}</p>}
    {health && <>
      <p>当前项目最近 {health.windowLimit} 个任务 · 实际 {health.sampleCount} 个记录，不代表完整历史。</p>
      <dl className="settings-list"><div><dt>活动 / 成功 / 失败 / 取消</dt><dd>{health.active} / {health.succeeded} / {health.failed} / {health.cancelled}</dd></div>
        <div><dt>遥测完整 / 部分 / 旧记录未采集 / 异常</dt><dd>{health.telemetryComplete} / {health.telemetryPartial} / {health.telemetryUnavailable} / {health.telemetryInvalid}</dd></div></dl>
      <h4>阶段耗时中位数</h4><dl className="settings-list">{([
        ["准备", health.prepare], ["提交", health.submit], ["排队", health.queueWait], ["执行", health.execution], ["收集", health.collection], ["总计", health.total],
      ] as [string, DiagnosticDurationSample][]).map(([label, sample]) => <div key={label}><dt>{label}</dt><dd>{durationLabel(sample.medianMs)} · {sample.sampleCount} 个有效样本</dd></div>)}</dl>
    </>}
    <h4>最近失败</h4>
    {!loading && !error && projectId && !failures.length && <p>该窗口没有已记录的失败。</p>}
    {failures.map(failure => <article className="technical-error-details" key={failure.code}>
      <p><code>{failure.code}</code> · {failure.count} 个任务 · {failure.latestAt ? formatDateTime(failure.latestAt) : "时间未记录"} · 失败</p>
      {failure.code === "UNKNOWN" && <p>原因未记录</p>}
      <button type="button" onClick={() => void openTimeline(failure.exampleTaskId)}>技术诊断</button>{" "}
      <button type="button" disabled={!props.onOpenRun} onClick={() => props.onOpenRun?.(failure.projectId, failure.exampleTaskId)}>查看运行</button>{" "}
      <button type="button" disabled={!props.onOpenAudit} onClick={() => props.onOpenAudit?.(failure.projectId, failure.exampleTaskId)}>查看生产链路</button>
    </article>)}
    <details open={!!initialTaskId}><summary>任务技术时间线</summary>
      <label>任务 ID <input value={taskId} onChange={event => setTaskId(event.target.value)} /></label>{" "}
      <button type="button" disabled={!projectId || !taskId.trim() || timelineLoading} onClick={() => void openTimeline(taskId.trim())}>读取任务诊断</button>
      {timelineLoading && <p role="status">正在读取任务时间线…</p>}{timelineError && <p role="alert">{timelineError}</p>}
      {timeline && <DiagnosticTimeline timeline={timeline} onOpenRun={props.onOpenRun} onOpenAudit={props.onOpenAudit} />}
    </details>
  </section>;
}

export function durationLabel(value: number | null): string {
  return value == null ? "未知" : value < 1000 ? `${value} ms` : `${(value / 1000).toFixed(1)} s`;
}

export function DiagnosticTimeline({ timeline: t, onOpenRun, onOpenAudit }: { timeline: DiagnosticTaskTimeline } & Pick<DiagnosticsExecutionProps, "onOpenRun" | "onOpenAudit">) {
  return <section className="technical-error-details" aria-label="任务技术诊断"><h4>任务时间线</h4>
    <p><code>{t.taskId}</code> · {t.status}</p>
    {t.completeness === "LEGACY_UNAVAILABLE" && <p>旧任务未采集完整执行遥测</p>}
    {t.completeness === "INVALID" && <p>遥测记录存在时间顺序异常；仅报告，不修改记录。</p>}
    {t.completeness === "PARTIAL" && <p>部分阶段尚未记录</p>}
    <dl className="settings-list">{([
      ["创建", t.createdAt], ["准备开始", t.prepareStartedAt], ["准备完成", t.preparedAt], ["提交", t.submittedAt],
      ["执行开始", t.executionStartedAt], ["执行完成", t.executionFinishedAt], ["收集完成", t.collectionFinishedAt], ["任务结束", t.finishedAt],
    ] as [string, string | null][]).map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value == null ? "—" : formatDateTime(value)}</dd></div>)}</dl>
    <dl className="settings-list">{([
      ["准备耗时", t.durations.prepareMs], ["提交耗时", t.durations.submitMs], ["排队耗时", t.durations.queueWaitMs],
      ["执行耗时", t.durations.executionMs], ["收集耗时", t.durations.collectionMs], ["总耗时", t.durations.totalMs],
    ] as [string, number | null][]).map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{durationLabel(value)}</dd></div>)}</dl>
    <p>执行关联：<code>{t.generationExecutionId ?? "未提供"}</code></p>
    <p>运行配置：{t.runtimeProfile ?? "未提供"} · 并发类别：{t.concurrencyClass ?? "未提供"}</p>
    <button type="button" disabled={!onOpenRun} onClick={() => onOpenRun?.(t.projectId, t.taskId)}>查看运行</button>{" "}
    <button type="button" disabled={!onOpenAudit} onClick={() => onOpenAudit?.(t.projectId, t.taskId)}>查看生产链路</button>
  </section>;
}
