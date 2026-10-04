import type { AppRoute } from "../../app/routes/types";
import { useWorkflowLabController,type WorkflowWorkspaceProps } from "../workflows/useWorkflowLabController";
import "./WorkflowLabPage.css";
import { LabDiagnosticsPane } from "./LabDiagnosticsPane";
import { LabBenchmarkPane } from "./LabBenchmarkPane";
import { useState } from "react";
import { toUserMessage } from "../../i18n/errorMessages";
import { WorkflowLabSurface } from "./WorkflowLabSurface";

/** Advanced view over existing Onboarding/Registry/Queue authorities, not a store. */
export function WorkflowLabPage({ returnTo, navigate, initialDiagnosticsOpen, ...props }: WorkflowWorkspaceProps & { returnTo?: AppRoute; navigate: (route: AppRoute) => unknown; initialDiagnosticsOpen?: boolean }) {
  const [actionError, setActionError] = useState<string>();
  const controller = useWorkflowLabController({ ...props, onOpenStudio: async (workflowId, recipeId) => {
    setActionError(undefined);
    try { await props.onOpenStudio(workflowId, recipeId); } catch (error) { setActionError(toUserMessage(error)); }
  }, onUseInProject: async (workflowId, recipeId) => {
    setActionError(undefined);
    try { await props.onUseInProject(workflowId, recipeId); } catch (error) { setActionError(toUserMessage(error)); }
  } });
  return <section className="workflow-lab" aria-label="高级工作流 Lab">
    <header><h1>高级工作流 Lab</h1><p>导入、映射、版本与诊断。高级验证仍使用现有生产队列；浏览不会修改项目绑定。</p>
      {returnTo && <button type="button" onClick={() => navigate(returnTo)}>返回原页面</button>}
    </header>
    {actionError && <p role="alert">{actionError}</p>}
    <WorkflowLabSurface controller={controller} />
    <details><summary>高级验证 · Benchmark</summary>{props.projectId ? <LabBenchmarkPane projectId={props.projectId} catalog={props.catalog} onOpenTask={props.onOpenTask} /> : <p>选择项目后才能运行高级验证。</p>}</details>
    <details open={initialDiagnosticsOpen}><summary>诊断与修复状态</summary><LabDiagnosticsPane items={controller.items} /></details>
  </section>;
}
