import { FormEvent, useState } from "react";
import { createProject, updateProject } from "../../services/tauriClient";
import type { ProjectView } from "../../types/project";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, projectDisplayName } from "../../i18n/statusLabels";

interface Props {
  projects: ProjectView[];
  activeProjectId?: string;
  onOpen: (projectId: string) => void;
  onProjectUpdated: (project: ProjectView) => void;
}

type FormMode = { kind: "create" } | { kind: "edit"; project: ProjectView };

export function ProjectWorkspace({ projects, activeProjectId, onOpen, onProjectUpdated }: Props) {
  const [formMode, setFormMode] = useState<FormMode>();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();

  function beginCreate() {
    setFormMode({ kind: "create" });
    setName("");
    setDescription("");
    setError(undefined);
  }

  function beginEdit(project: ProjectView) {
    setFormMode({ kind: "edit", project });
    setName(project.name);
    setDescription(project.description ?? "");
    setError(undefined);
  }

  function closeForm() {
    if (saving) return;
    setFormMode(undefined);
    setError(undefined);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSaving(true);
    setError(undefined);
    const normalizedDescription = description.trim() || undefined;
    try {
      const project = formMode?.kind === "edit"
        ? await updateProject(formMode.project.id, name, normalizedDescription)
        : await createProject(name, normalizedDescription);
      onProjectUpdated(project);
      setFormMode(undefined);
      if (formMode?.kind === "create") onOpen(project.id);
    } catch (saveError: unknown) {
      setError(toUserMessage(saveError));
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="workspace-panel project-workspace" aria-busy={saving}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">工作区</span>
          <h2>项目</h2>
          <p className="section-description">将任务和资产整理到本地项目中。</p>
        </div>
        <button type="button" onClick={beginCreate} disabled={saving}>新建项目</button>
      </div>

      {formMode && (
        <form className="project-form" onSubmit={(event) => void submit(event)}>
          <div className="section-heading">
            <div>
              <span className="section-label">{formMode.kind === "create" ? "创建" : "编辑"}</span>
              <h3>{formMode.kind === "create" ? "新建项目" : "项目详情"}</h3>
            </div>
          </div>
          <label>
            <span>项目名称</span>
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              maxLength={80}
              required
              autoFocus
            />
          </label>
          <label>
            <span>项目说明 <small>可选</small></span>
            <textarea
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              maxLength={500}
              rows={3}
            />
          </label>
          {error && <p className="error-message" role="alert">{error}</p>}
          <div className="project-form-actions">
            <button type="submit" disabled={saving}>{saving ? "正在保存..." : formMode.kind === "create" ? "创建项目" : "保存修改"}</button>
            <button type="button" className="quiet-button" onClick={closeForm} disabled={saving}>取消</button>
          </div>
        </form>
      )}

      <div className="project-table" role="table" aria-label="项目列表">
        <div className="project-table-row project-table-header" role="row">
          <span role="columnheader">名称</span>
          <span role="columnheader">说明</span>
          <span role="columnheader">创建时间</span>
          <span role="columnheader">更新时间</span>
          <span role="columnheader">状态</span>
          <span role="columnheader">操作</span>
        </div>
        {projects.map((project) => {
          const active = project.id === activeProjectId;
          return (
            <div className="project-table-row" role="row" key={project.id}>
              <strong role="cell">{projectDisplayName(project.id, project.name)}</strong>
              <span role="cell" className="project-description-cell">{project.description || "—"}</span>
              <span role="cell">{formatDateTime(project.createdAt)}</span>
              <span role="cell">{formatDateTime(project.updatedAt)}</span>
              <span role="cell">{active ? <span className="active-project-badge">当前项目</span> : ""}</span>
              <span role="cell" className="project-row-actions">
                <button type="button" onClick={() => onOpen(project.id)} disabled={active || saving}>
                  打开
                </button>
                <button type="button" className="quiet-button" onClick={() => beginEdit(project)} disabled={saving}>编辑</button>
              </span>
            </div>
          );
        })}
        {!projects.length && <p className="empty-state">暂无项目。</p>}
      </div>
    </section>
  );
}
