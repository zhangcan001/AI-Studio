CREATE TABLE prompt_entries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('prompt', 'snippet')),
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    tags_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    UNIQUE(project_id, kind, normalized_name)
);

CREATE INDEX idx_prompt_entries_project_updated
ON prompt_entries(project_id, updated_at DESC, id ASC);

CREATE INDEX idx_prompt_entries_project_kind
ON prompt_entries(project_id, kind, updated_at DESC, id ASC);

CREATE TABLE prompt_versions (
    id TEXT PRIMARY KEY,
    prompt_id TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    text TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (prompt_id) REFERENCES prompt_entries(id) ON DELETE CASCADE,
    UNIQUE(prompt_id, version)
);

CREATE INDEX idx_prompt_versions_prompt_version
ON prompt_versions(prompt_id, version DESC);
