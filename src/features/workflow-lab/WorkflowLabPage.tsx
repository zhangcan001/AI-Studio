import type { AppRoute } from "../../app/routes/types";
import "./WorkflowLabPage.css";

interface Props { projectId?: string; returnTo?: AppRoute; navigate: (route: AppRoute) => unknown }

/** Compatibility destination for saved routes only. Never loads authoring IPC. */
export function WorkflowLabPage({ projectId, returnTo, navigate }: Props) {
  // Do not let a legacy return locator bounce back into the retired surface.
  const canReturn = returnTo && !((returnTo.kind === "system-settings" || returnTo.kind === "project-settings") && returnTo.section === "advanced-workflows");
  return <section className="workflow-lab" aria-label="工作流入口已停用">
    <header><h1>工作流导入与编辑已停用</h1>
      <p>请在项目生成器设置中选择内置工作流。已有绑定和历史记录不受影响。</p>
      {projectId ? <button type="button" onClick={() => navigate({ kind: "project-settings", projectId, section: "generators" })}>选择项目生成器</button> : <button type="button" onClick={() => navigate({ kind: "project-list" })}>选择项目</button>}
      {canReturn && <button type="button" onClick={() => navigate(returnTo)}>返回原页面</button>}
    </header>
  </section>;
}
