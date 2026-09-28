import { useEffect, useState } from "react";
import { repairJobsStatus } from "../../services/tauriClient";
import type { RepairJobStatus, RepairJobStatusView } from "../../types/repairJobs";
import { formatDateTime } from "../../i18n/statusLabels";
import { formatUiError } from "../../i18n/errorMessages";

interface ListProps {
  jobs?: RepairJobStatusView[];
  loading: boolean;
  error?: unknown;
}

export function repairJobStatusLabel(status: RepairJobStatus): string {
  switch (status) {
    case "COMPLETED": return "已完成";
    case "RUNNING": return "运行中";
    case "FAILED": return "失败";
    case "SKIPPED": return "已跳过";
    default: return status;
  }
}

/** Read-only presentation of the app-level repair job markers. */
export function RepairJobsStatusList({ jobs, loading, error }: ListProps) {
  const repaired = (jobs ?? []).reduce((total, job) => total + (job.summary?.repaired ?? 0), 0);
  const needsReview = (jobs ?? []).reduce((total, job) => total + (job.summary?.needsReview.length ?? 0), 0);
  return (
    <section className="settings-card" aria-labelledby="settings-repair-jobs-title">
      <div className="settings-card-heading">
        <div>
          <h3 id="settings-repair-jobs-title">数据修复任务</h3>
          <p>应用升级后自动运行一次，此处只读显示结果。</p>
        </div>
      </div>
      {loading && <p className="settings-notice">正在读取修复状态…</p>}
      {Boolean(error) && <p className="settings-warning">修复状态读取失败：{formatUiError(error).message}</p>}
      {!loading && !error && jobs && (
        jobs.length === 0 ? <p className="settings-notice">暂无修复任务记录。</p> : (
          <>
            <p className="settings-notice">已自动修复 {repaired} 个配方{needsReview ? ` · ${needsReview} 项需人工检查` : ""}</p>
            <ul className="settings-repair-jobs">
              {jobs.map((job) => (
                <li key={job.jobId} className={job.status === "FAILED" ? "settings-warning" : undefined}>
                  <code>{job.jobId}</code> · {repairJobStatusLabel(job.status)}
                  {job.summary ? ` · 修复 ${job.summary.repaired} / 计划 ${job.summary.planned}` : ""}
                  {job.summary?.failed.length ? ` · 失败 ${job.summary.failed.length}` : ""}
                  {job.completedAt ? ` · ${formatDateTime(job.completedAt)}` : ""}
                </li>
              ))}
            </ul>
          </>
        )
      )}
    </section>
  );
}

export function RepairJobsStatusSection() {
  const [jobs, setJobs] = useState<RepairJobStatusView[]>();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<unknown>();
  useEffect(() => {
    let cancelled = false;
    repairJobsStatus()
      .then((next) => { if (!cancelled) setJobs(next); })
      .catch((nextError: unknown) => { if (!cancelled) setError(nextError); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, []);
  return <RepairJobsStatusList jobs={jobs} loading={loading} error={error} />;
}
