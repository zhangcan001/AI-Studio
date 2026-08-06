import { FormEvent, useState } from "react";
import { createProject, updateProject } from "../../services/tauriClient";
import type { ProjectView } from "../../types/project";

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
      setError(saveError instanceof Error ? saveError.message : String(saveError));
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="workspace-panel project-workspace" aria-busy={saving}>
      <div className="section-heading workspace-heading">
        <div>
          <span className="section-label">Workspace</span>
          <h2>Projects</h2>
          <p className="section-description">Organize tasks and assets into local project contexts.</p>
        </div>
        <button type="button" onClick={beginCreate} disabled={saving}>New Project</button>
      </div>

      {formMode && (
        <form className="project-form" onSubmit={(event) => void submit(event)}>
          <div className="section-heading">
            <div>
              <span className="section-label">{formMode.kind === "create" ? "Create" : "Edit"}</span>
              <h3>{formMode.kind === "create" ? "New project" : "Project details"}</h3>
            </div>
          </div>
          <label>
            <span>Name</span>
            <input
              value={name}
              onChange={(event) => setName(event.target.value)}
              maxLength={80}
              required
              autoFocus
            />
          </label>
          <label>
            <span>Description <small>Optional</small></span>
            <textarea
              value={description}
              onChange={(event) => setDescription(event.target.value)}
              maxLength={500}
              rows={3}
            />
          </label>
          {error && <p className="error-message" role="alert">{error}</p>}
          <div className="project-form-actions">
            <button type="submit" disabled={saving}>{saving ? "Saving..." : formMode.kind === "create" ? "Create project" : "Save changes"}</button>
            <button type="button" className="quiet-button" onClick={closeForm} disabled={saving}>Cancel</button>
          </div>
        </form>
      )}

      <div className="project-table" role="table" aria-label="Projects">
        <div className="project-table-row project-table-header" role="row">
          <span role="columnheader">Name</span>
          <span role="columnheader">Description</span>
          <span role="columnheader">Created</span>
          <span role="columnheader">Updated</span>
          <span role="columnheader">Status</span>
          <span role="columnheader">Actions</span>
        </div>
        {projects.map((project) => {
          const active = project.id === activeProjectId;
          return (
            <div className="project-table-row" role="row" key={project.id}>
              <strong role="cell">{project.name}</strong>
              <span role="cell" className="project-description-cell">{project.description || "—"}</span>
              <span role="cell">{formatDate(project.createdAt)}</span>
              <span role="cell">{formatDate(project.updatedAt)}</span>
              <span role="cell">{active ? <span className="active-project-badge">Active</span> : ""}</span>
              <span role="cell" className="project-row-actions">
                <button type="button" onClick={() => onOpen(project.id)} disabled={active || saving}>
                  Open
                </button>
                <button type="button" className="quiet-button" onClick={() => beginEdit(project)} disabled={saving}>Edit</button>
              </span>
            </div>
          );
        })}
        {!projects.length && <p className="empty-state">No projects are available.</p>}
      </div>
    </section>
  );
}

function formatDate(value: string): string {
  return new Date(value).toLocaleString();
}
