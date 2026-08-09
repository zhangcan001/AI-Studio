CREATE TABLE asset_tags (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id),
    UNIQUE(project_id, normalized_name)
);

CREATE INDEX idx_asset_tags_project
ON asset_tags(project_id, created_at ASC, id ASC);

CREATE INDEX idx_asset_tags_normalized_name
ON asset_tags(normalized_name);

CREATE TABLE asset_tag_links (
    asset_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(asset_id, tag_id),
    FOREIGN KEY (asset_id) REFERENCES assets(id),
    FOREIGN KEY (tag_id) REFERENCES asset_tags(id),
    FOREIGN KEY (project_id) REFERENCES projects(id)
);

CREATE INDEX idx_asset_tag_links_project_tag
ON asset_tag_links(project_id, tag_id, asset_id);

CREATE TABLE asset_favorites (
    asset_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (asset_id) REFERENCES assets(id),
    FOREIGN KEY (project_id) REFERENCES projects(id)
);

CREATE INDEX idx_asset_favorites_project
ON asset_favorites(project_id, created_at DESC, asset_id);

CREATE TABLE project_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    description TEXT,
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    values_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (workflow_version_id) REFERENCES workflow_versions(id),
    FOREIGN KEY (recipe_id) REFERENCES recipes(id),
    UNIQUE(normalized_name)
);

CREATE INDEX idx_project_templates_updated
ON project_templates(updated_at DESC, id ASC);
