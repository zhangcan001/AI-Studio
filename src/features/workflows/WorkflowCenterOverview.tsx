import type { RecipeViewModel } from "../../types/generation";
import type { WorkflowCenterSummary, WorkflowProductionProfile } from "./workflowCenterModel";
import { WorkflowProductionProfiles } from "./WorkflowProductionProfiles";

interface Props {
  summary: WorkflowCenterSummary;
  profiles: WorkflowProductionProfile[];
  projectId?: string;
  comfyConnected: boolean;
  workspaceLoading: boolean;
  projectConfigLoading: boolean;
  projectConfigError?: string;
  runtimeProfilesLoading: boolean;
  runtimeProfilesError?: string;
  onOpenProjectSettings: () => void;
  onManageParameters: (recipe: RecipeViewModel) => void;
}

export function WorkflowCenterOverview({
  summary,
  profiles,
  projectId,
  comfyConnected,
  workspaceLoading,
  projectConfigLoading,
  projectConfigError,
  runtimeProfilesLoading,
  runtimeProfilesError,
  onOpenProjectSettings,
  onManageParameters,
}: Props) {
  return (
    <section className="workflow-center-overview" aria-labelledby="workflow-center-overview-title">
      <div className="section-heading">
        <div>
          <span className="section-label">总览</span>
          <h2 id="workflow-center-overview-title">工作流中心</h2>
          <p className="section-description">先看哪些工作流可以生产、当前项目正在使用什么，以及下一步该处理什么。</p>
        </div>
        <span className={`workflow-center-connection workflow-center-connection-${comfyConnected ? "ready" : "attention"}`}>
          ComfyUI：{comfyConnected ? "已连接" : "运行检查暂不可用"}
        </span>
      </div>
      <div className="workflow-center-summary" aria-label="工作流中心概览指标">
        <div><span>可生产工作流</span><strong>{summary.availableWorkflowCount}</strong></div>
        <div><span>需处理工作流</span><strong>{summary.issueWorkflowCount}</strong></div>
        <div><span>当前项目已使用</span><strong>{summary.projectUsedWorkflowCount}</strong></div>
        <div><span>运行参数档案</span><strong>{summary.runtimeProfileCount}</strong></div>
      </div>
      {workspaceLoading && <p className="loading-state">正在读取工作流…</p>}
      <section className="workflow-center-project-context" aria-label="当前项目生产配置">
        <div className="section-heading">
          <div>
            <span className="section-label">项目上下文</span>
            <h3>{projectId ? "当前项目生产配置" : "选择项目后查看生产配置"}</h3>
          </div>
          <button type="button" className="quiet-button" onClick={onOpenProjectSettings}>
            {projectId ? "配置工作流" : "选择项目"}
          </button>
        </div>
        {!projectId && <p className="disabled-note">工作流仍可管理；选择项目后可查看图片、视频和模式专用生产路径。</p>}
        {projectId && projectConfigLoading && <p className="loading-state">正在读取项目配置…</p>}
        {projectId && projectConfigError && <p className="error-message" role="alert">项目配置读取失败，工作流列表仍可继续使用：{projectConfigError}</p>}
        {projectId && !projectConfigLoading && !projectConfigError && (
          <WorkflowProductionProfiles
            profiles={profiles}
            onChangeWorkflow={onOpenProjectSettings}
            onManageParameters={onManageParameters}
          />
        )}
        {runtimeProfilesLoading && <p className="disabled-note">正在读取运行参数档案…</p>}
        {runtimeProfilesError && <p className="settings-warning">运行参数档案暂时不可用；工作流生产配置仍可查看。</p>}
      </section>
    </section>
  );
}
