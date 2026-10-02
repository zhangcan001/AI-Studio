import { useEffect, useState } from "react";
import { repairJobsStatus } from "../../services/workflowLabClient";
import { RepairJobsStatusList } from "../settings/RepairJobsStatusSection";
import type { RepairJobStatusView } from "../../types/repairJobs";
import type { WorkflowWorkspaceItem } from "../workflows/workflowWorkspaceAdapters";
type DiagnosticItem = Pick<WorkflowWorkspaceItem, "workflowVersionId" | "recipeId" | "packageName" | "packageSourcePath" | "workflowSha256" | "recipeSha256" | "diagnostics" | "capability" | "readiness">;
export function LabDiagnosticsPane({ items = [] }: { items?: DiagnosticItem[] }) {
  const [jobs, setJobs] = useState<RepairJobStatusView[]>();
  const [error, setError] = useState<unknown>();
  useEffect(() => { let alive = true; void repairJobsStatus().then(j => { if (alive) setJobs(j); }).catch(e => { if (alive) setError(e); }); return () => { alive = false; }; }, []);
  return <section aria-label="Lab 诊断"><p>版本详情和历史中可查看确切版本、配方、节点、证据与运行阻塞项。修复由现有后台任务负责，这里只读显示。</p>
    {items.map(item => <details key={JSON.stringify([item.workflowVersionId, item.recipeId, item.packageName])}>
      <summary>{item.packageName} · 运行包技术详情</summary>
      <dl><dt>WorkflowVersion ID</dt><dd><code>{item.workflowVersionId || "未返回"}</code></dd>
        <dt>Recipe ID</dt><dd><code>{item.recipeId || "未返回"}</code></dd>
        <dt>运行包路径</dt><dd><code>{item.packageSourcePath || "未返回"}</code></dd>
        <dt>Workflow SHA-256</dt><dd><code>{item.workflowSha256 || "未返回"}</code></dd>
        <dt>Recipe SHA-256</dt><dd><code>{item.recipeSha256 || "未返回"}</code></dd>
        <dt>运行状态</dt><dd>{item.capability} · {item.readiness}</dd></dl>
      <ul>{item.diagnostics.map(diagnostic => <li key={diagnostic.code}><code>{diagnostic.code}</code> · {diagnostic.message}</li>)}</ul>
    </details>)}
    <RepairJobsStatusList jobs={jobs} error={error} loading={!jobs && !error} /></section>;
}
