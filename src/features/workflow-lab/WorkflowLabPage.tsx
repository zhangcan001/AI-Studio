import type { AppRoute } from "../../app/routes/types";
import type { WorkflowWorkspaceProps } from "../workflows/useWorkflowLabController";
import "./WorkflowLabPage.css";

/** Compatibility destination for saved routes only. Never loads authoring IPC. */
export function WorkflowLabPage({ projectId, returnTo, navigate }: WorkflowWorkspaceProps & { returnTo?: AppRoute; navigate: (route: AppRoute) => unknown; initialDiagnosticsOpen?: boolean }) {
  // Do not let a legacy return locator bounce back into the retired surface.
  const canReturn = returnTo && !((returnTo.kind === "system-settings" || returnTo.kind === "project-settings") && returnTo.section === "advanced-workflows");
  return <section className="workflow-lab" aria-label="工作流入口已停用">
    <header><h1>工作流导入与编辑已停用</h1>
      <p>新增工作流将通过软件内置的生成器提供，不再直接导入、自动识别或手动映射工作流。</p>
      <p>已有项目绑定、历史任务、生成结果与备份保持不变；不会自动替换生成器或启动任务。</p>
      {projectId ? <button type="button" onClick={() => navigate({ kind: "project-settings", projectId, section: "generators" })}>选择项目生成器</button> : <button type="button" onClick={() => navigate({ kind: "project-list" })}>选择项目</button>}
      {canReturn && <button type="button" onClick={() => navigate(returnTo)}>返回原页面</button>}
    </header>
  </section>;
}
