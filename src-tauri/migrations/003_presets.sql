CREATE TABLE presets (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    name TEXT NOT NULL,
    values_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    FOREIGN KEY (project_id)
        REFERENCES projects(id),

    FOREIGN KEY (workflow_version_id)
        REFERENCES workflow_versions(id),

    FOREIGN KEY (recipe_id)
        REFERENCES recipes(id),

    UNIQUE(project_id, workflow_version_id, recipe_id, name)
);

CREATE INDEX idx_presets_project_recipe
ON presets(project_id, workflow_version_id, recipe_id, updated_at DESC);
