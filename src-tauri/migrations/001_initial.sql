CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    root_path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE workflows (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    mode TEXT NOT NULL,
    current_version_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE workflow_versions (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL,
    version TEXT NOT NULL,
    api_workflow_json TEXT NOT NULL,
    workflow_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL,

    FOREIGN KEY (workflow_id)
        REFERENCES workflows(id),

    UNIQUE(workflow_id, version)
);

CREATE TABLE recipes (
    id TEXT PRIMARY KEY,
    workflow_version_id TEXT NOT NULL,
    version TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    recipe_yaml TEXT NOT NULL,
    recipe_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL,

    FOREIGN KEY (workflow_version_id)
        REFERENCES workflow_versions(id)
);

CREATE TABLE tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    workflow_id TEXT NOT NULL,
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,

    status TEXT NOT NULL,

    prompt_id TEXT,
    queue_number INTEGER,

    progress_mode TEXT NOT NULL DEFAULT 'indeterminate',
    progress_current INTEGER,
    progress_total INTEGER,

    current_node_id TEXT,

    error_code TEXT,
    error_message TEXT,
    raw_error_json TEXT,

    created_at TEXT NOT NULL,
    queued_at TEXT,
    started_at TEXT,
    finished_at TEXT,

    FOREIGN KEY (project_id)
        REFERENCES projects(id),

    FOREIGN KEY (workflow_id)
        REFERENCES workflows(id),

    FOREIGN KEY (workflow_version_id)
        REFERENCES workflow_versions(id),

    FOREIGN KEY (recipe_id)
        REFERENCES recipes(id)
);

CREATE TABLE assets (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,

    type TEXT NOT NULL,
    category TEXT,

    name TEXT NOT NULL,
    original_name TEXT,

    storage_path TEXT NOT NULL,
    thumbnail_path TEXT,

    sha256 TEXT NOT NULL,
    mime_type TEXT,

    width INTEGER,
    height INTEGER,

    file_size INTEGER,

    source_task_id TEXT,

    metadata_json TEXT,

    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    FOREIGN KEY (project_id)
        REFERENCES projects(id),

    FOREIGN KEY (source_task_id)
        REFERENCES tasks(id)
);

CREATE TABLE generation_snapshots (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL UNIQUE,

    workflow_json TEXT NOT NULL,
    recipe_yaml TEXT NOT NULL,

    user_inputs_json TEXT NOT NULL,
    resolved_inputs_json TEXT NOT NULL,

    created_at TEXT NOT NULL,

    FOREIGN KEY (task_id)
        REFERENCES tasks(id)
);

CREATE TABLE task_events (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL,

    sequence INTEGER NOT NULL,
    event_type TEXT NOT NULL,
    payload_json TEXT,

    created_at TEXT NOT NULL,

    FOREIGN KEY (task_id)
        REFERENCES tasks(id),

    UNIQUE(task_id, sequence)
);

CREATE INDEX idx_tasks_created_at
ON tasks(created_at DESC);

CREATE INDEX idx_tasks_status
ON tasks(status);

CREATE INDEX idx_assets_project
ON assets(project_id);

CREATE INDEX idx_assets_source_task
ON assets(source_task_id);

CREATE INDEX idx_task_events_task
ON task_events(task_id, sequence);
