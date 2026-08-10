import { FormEvent, useEffect, useState } from "react";
import {
  createProject,
  exportProjectBackup,
  inspectProjectBackup,
  restoreProjectBackup,
  updateProject,
  createProjectFromTemplate,
  deleteProjectTemplate,
  listProjectTemplates,
  updateProjectTemplate,
} from "../../services/tauriClient";
import type { ProjectBackupPreview, ProjectView } from "../../types/project";
import { toUserMessage } from "../../i18n/errorMessages";
import { formatDateTime, projectDisplayName } from "../../i18n/statusLabels";
import type { ProjectTemplate, TemplateProjectResult } from "../../types/organization";

interface Props {
  projects: ProjectView[];
  activeProjectId?: string;
  onOpen: (projectId: string) => void;
  onProjectUpdated: (project: ProjectView) => void;
  onProjectRestored: (project: ProjectView) => void;
  onTemplateProjectCreated: (result: TemplateProjectResult) => void;
}

type FormMode = { kind: "create" } | { kind: "edit"; project: ProjectView };

export function ProjectWorkspace({ projects, activeProjectId, onOpen, onProjectUpdated, onProjectRestored, onTemplateProjectCreated }: Props) {
  const [formMode, setFormMode] = useState<FormMode>();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string>();
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupPreview, setBackupPreview] = useState<ProjectBackupPreview>();
  const [backupNotice, setBackupNotice] = useState<string>();
  const [templates, setTemplates] = useState<ProjectTemplate[]>([]);
  const [templateBusy, setTemplateBusy] = useState(false);
  const [templateForm, setTemplateForm] = useState<{ kind: "edit" | "createProject"; template: ProjectTemplate }>();
  const [templateName, setTemplateName] = useState("");
  const [templateDescription, setTemplateDescription] = useState("");

  async function reloadTemplates() {
    setTemplates(await listProjectTemplates());
  }

  useEffect(() => { void reloadTemplates().catch((value) => setError(toUserMessage(value))); }, []);

  function beginTemplateForm(kind: "edit" | "createProject", template: ProjectTemplate) {
    setTemplateForm({ kind, template });
    setTemplateName(kind === "edit" ? template.name : `${template.name} 项目`);
    setTemplateDescription(template.description ?? "");
    setError(undefined);
  }

  async function submitTemplateForm(event: FormEvent) {
    event.preventDefault();
    if (!templateForm) return;
    setTemplateBusy(true); setError(undefined);
    try {
      if (templateForm.kind === "edit") {
        await updateProjectTemplate(templateForm.template.id, templateName, templateDescription.trim() || undefined);
        await reloadTemplates();
      } else {
        const result = await createProjectFromTemplate(templateForm.template.id, templateName, templateDescription.trim() || undefined);
        onProjectUpdated(result.project);
        onTemplateProjectCreated(result);
      }
      setTemplateForm(undefined);
    } catch (value) { setError(toUserMessage(value)); } finally { setTemplateBusy(false); }
  }

  async function removeTemplate(template: ProjectTemplate) {
    if (!window.confirm(`删除项目模板“${template.name}”？此操作不会删除项目、工作流、预设或素材。`)) return;
    setTemplateBusy(true); setError(undefined);
    try { await deleteProjectTemplate(template.id); await reloadTemplates(); }
    catch (value) { setError(toUserMessage(value)); } finally { setTemplateBusy(false); }
  }

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

  async function exportBackup(projectId: string) {
    setBackupBusy(true);
    setError(undefined);
    setBackupNotice(undefined);
    try {
      const exported = await exportProjectBackup(projectId);
      if (exported) {
        setBackupNotice(`项目备份已保存：${exported.fileName}（${exported.entries} 个文件）`);
      }
    } catch (backupError: unknown) {
      setError(toUserMessage(backupError));
    } finally {
      setBackupBusy(false);
    }
  }

  async function inspectBackup() {
    setBackupBusy(true);
    setError(undefined);
    setBackupNotice(undefined);
    try {
      const preview = await inspectProjectBackup();
      if (preview) setBackupPreview(preview);
    } catch (backupError: unknown) {
      setError(toUserMessage(backupError));
    } finally {
      setBackupBusy(false);
    }
  }

  async function restoreBackup() {
    if (!backupPreview) return;
    if (!window.confirm(`确认恢复项目“${backupPreview.projectName}”？恢复后会创建一个新项目，不会自动生成。`)) return;
    setBackupBusy(true);
    setError(undefined);
    try {
      const restored = await restoreProjectBackup(backupPreview.inspectionId);
      setBackupPreview(undefined);
      setBackupNotice(`项目已恢复：${restored.name}`);
      onProjectRestored(restored);
    } catch (backupError: unknown) {
      setError(toUserMessage(backupError));
    } finally {
      setBackupBusy(false);
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
        <div className="project-heading-actions">
          <button type="button" onClick={() => activeProjectId && void exportBackup(activeProjectId)} disabled={saving || backupBusy || !activeProjectId}>导出备份</button>
          <button type="button" onClick={() => void inspectBackup()} disabled={saving || backupBusy}>恢复项目</button>
          <button type="button" onClick={beginCreate} disabled={saving || backupBusy}>新建项目</button>
        </div>
      </div>

      {backupNotice && <p className="settings-notice" role="status">{backupNotice}</p>}
      {error && !formMode && <p className="error-message" role="alert">{error}</p>}
      {backupPreview && (
        <section className="project-backup-preview" aria-labelledby="project-backup-preview-title">
          <div className="section-heading">
            <div>
              <span className="section-label">备份预览</span>
              <h3 id="project-backup-preview-title">{backupPreview.projectName}</h3>
            </div>
            <button type="button" className="quiet-button" onClick={() => setBackupPreview(undefined)} disabled={backupBusy}>取消</button>
          </div>
          <p>图片 {backupPreview.imageCount} · 视频 {backupPreview.videoCount} · 音频 {backupPreview.audioCount} · 历史任务 {backupPreview.historyTasks} · 预设 {backupPreview.presets} · 生产队列 {backupPreview.productionQueues} · 镜头 {backupPreview.shots ?? 0}</p>
          {backupPreview.missingWorkflows.length > 0 && <p className="error-message">缺少工作流：{backupPreview.missingWorkflows.join("、")}；历史记录仍可恢复。</p>}
          <p className="settings-warning">{backupPreview.warning}</p>
          <button type="button" className="primary-action" onClick={() => void restoreBackup()} disabled={backupBusy}>
            {backupBusy ? "正在恢复……" : "确认恢复项目"}
          </button>
        </section>
      )}

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

      <section className="project-templates" aria-labelledby="project-templates-title">
        <div className="section-heading"><div><span className="section-label">可复用创作起点</span><h3 id="project-templates-title">项目模板</h3><p className="section-description">从已保存的工作流和无素材参数创建新项目。</p></div></div>
        <div className="project-template-grid">
          {templates.map((template) => <article key={template.id} className="project-template-card">
            <div><strong>{template.name}</strong><p>{template.description || "暂无说明"}</p><small>{template.available ? "工作流可用" : "工作流当前不可用"}</small></div>
            <div><button type="button" onClick={() => beginTemplateForm("createProject", template)} disabled={templateBusy || !template.available}>从模板新建</button><button type="button" className="quiet-button" onClick={() => beginTemplateForm("edit", template)} disabled={templateBusy}>编辑</button><button type="button" className="quiet-button" onClick={() => void removeTemplate(template)} disabled={templateBusy}>删除</button></div>
          </article>)}
          {!templates.length && <p className="empty-state">尚未保存项目模板。请在创作页保存当前草稿。</p>}
        </div>
        {templateForm && <form className="project-form project-template-form" onSubmit={(event) => void submitTemplateForm(event)}>
          <div className="section-heading"><div><span className="section-label">{templateForm.kind === "edit" ? "编辑模板" : "创建新项目"}</span><h3>{templateForm.template.name}</h3></div></div>
          <label><span>{templateForm.kind === "edit" ? "模板名称" : "项目名称"}</span><input autoFocus required maxLength={80} value={templateName} onChange={(event) => setTemplateName(event.target.value)} /></label>
          <label><span>{templateForm.kind === "edit" ? "模板说明" : "项目说明"} <small>可选</small></span><textarea rows={3} maxLength={500} value={templateDescription} onChange={(event) => setTemplateDescription(event.target.value)} /></label>
          <div className="project-form-actions"><button type="submit" disabled={templateBusy || !templateName.trim()}>{templateBusy ? "正在保存..." : templateForm.kind === "edit" ? "保存修改" : "创建并打开"}</button><button type="button" className="quiet-button" onClick={() => setTemplateForm(undefined)} disabled={templateBusy}>取消</button></div>
        </form>}
      </section>

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
                <button type="button" onClick={() => onOpen(project.id)} disabled={active || saving || backupBusy}>
                  打开
                </button>
                <button type="button" className="quiet-button" onClick={() => beginEdit(project)} disabled={saving || backupBusy}>编辑</button>
                <button type="button" className="quiet-button" onClick={() => void exportBackup(project.id)} disabled={saving || backupBusy}>导出备份</button>
              </span>
            </div>
          );
        })}
        {!projects.length && <p className="empty-state">暂无项目。</p>}
      </div>
    </section>
  );
}
