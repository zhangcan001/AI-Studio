CREATE TABLE production_batches (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL,
    continue_on_failure INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX idx_production_batches_project_updated
    ON production_batches(project_id, updated_at DESC, id ASC);

CREATE INDEX idx_production_batches_status
    ON production_batches(status, updated_at ASC);

CREATE TABLE production_batch_items (
    id TEXT PRIMARY KEY NOT NULL,
    batch_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    values_json TEXT NOT NULL,
    status TEXT NOT NULL,
    task_id TEXT,
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (batch_id) REFERENCES production_batches(id) ON DELETE CASCADE,
    FOREIGN KEY (workflow_version_id) REFERENCES workflow_versions(id),
    FOREIGN KEY (recipe_id) REFERENCES recipes(id),
    FOREIGN KEY (task_id) REFERENCES tasks(id),
    UNIQUE(batch_id, ordinal)
);

CREATE INDEX idx_production_batch_items_batch_ordinal
    ON production_batch_items(batch_id, ordinal ASC);

CREATE INDEX idx_production_batch_items_task
    ON production_batch_items(task_id);
