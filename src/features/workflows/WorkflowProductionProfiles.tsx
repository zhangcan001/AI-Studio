import type { RecipeViewModel } from "../../types/generation";
import type { WorkflowProductionProfile } from "./workflowCenterModel";

interface Props {
  profiles: WorkflowProductionProfile[];
  onChangeWorkflow: () => void;
  onManageParameters: (recipe: RecipeViewModel) => void;
}

const STATUS_LABELS: Record<WorkflowProductionProfile["status"], string> = {
  READY: "可生产",
  ATTENTION: "需要检查",
  BLOCKED: "不可用",
};

export function WorkflowProductionProfiles({ profiles, onChangeWorkflow, onManageParameters }: Props) {
  return (
    <section className="workflow-production-profiles" aria-labelledby="workflow-production-profiles-title">
      <div className="section-heading">
        <div>
          <span className="section-label">当前项目生产配置</span>
          <h3 id="workflow-production-profiles-title">可复用的生产路径</h3>
          <p className="section-description">状态、来源和参数档案均由现有项目绑定、工作流目录与运行事实实时派生。</p>
        </div>
      </div>
      <div className="workflow-production-profile-grid">
        {profiles.map((profile) => (
          <article className={`workflow-production-profile workflow-production-profile-${profile.status.toLowerCase()}`} key={profile.key}>
            <div className="workflow-production-profile-heading">
              <div>
                <span className="workflow-production-profile-purpose">{profile.purpose}</span>
                <h4>{profile.label}</h4>
              </div>
              <strong className="workflow-production-profile-status">{STATUS_LABELS[profile.status]}</strong>
            </div>
            <dl className="workflow-production-profile-facts">
              <div><dt>工作流</dt><dd>工作流：{profile.recipe?.name ?? "尚未配置"}</dd></div>
              <div><dt>来源</dt><dd>{profile.sourceLabel}</dd></div>
              <div><dt>参数档案</dt><dd>{profile.runtimeProfileCount} 个</dd></div>
            </dl>
            {profile.usingFallback && <p className="workflow-production-profile-fallback">当前使用{profile.sourceLabel}，不是已保存的项目默认。</p>}
            {profile.staleConfiguredBinding && <p className="workflow-production-profile-warning" role="alert">⚠ 项目绑定已失效，不会静默修改；请重新选择或清除绑定。</p>}
            {!profile.staleConfiguredBinding && profile.status !== "READY" && <p className="workflow-production-profile-warning">{profile.issue}</p>}
            <div className="workflow-production-profile-actions">
              <button type="button" onClick={onChangeWorkflow}>更改工作流</button>
              {profile.recipe && <button type="button" className="quiet-button" onClick={() => onManageParameters(profile.recipe!)}>管理参数</button>}
              {profile.technicalIdentity && (
                <details className="workflow-production-profile-technical">
                  <summary>查看技术详情</summary>
                  <small>WorkflowVersion：{profile.technicalIdentity.workflowVersionId}</small>
                  <small>Recipe：{profile.technicalIdentity.recipeId}</small>
                </details>
              )}
            </div>
          </article>
        ))}
        {!profiles.length && <p className="empty-state">当前项目还没有可展示的生产配置。</p>}
      </div>
    </section>
  );
}
