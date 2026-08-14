CREATE TABLE benchmark_experiments (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    media_type TEXT NOT NULL CHECK (media_type IN ('IMAGE', 'VIDEO')),
    status TEXT NOT NULL CHECK (status IN ('DRAFT', 'QUEUED', 'RUNNING', 'COMPLETED', 'PARTIAL', 'CANCELLED', 'FAILED_TO_QUEUE')),
    base_values_json TEXT NOT NULL,
    asset_ids_json TEXT NOT NULL,
    winner_candidate_id TEXT,
    production_batch_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (production_batch_id) REFERENCES production_batches(id) ON DELETE SET NULL
);

CREATE INDEX idx_benchmark_experiments_project_created
    ON benchmark_experiments(project_id, created_at DESC, id ASC);

CREATE TABLE benchmark_candidates (
    id TEXT PRIMARY KEY NOT NULL,
    experiment_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    preset_id TEXT,
    preset_name TEXT,
    label TEXT NOT NULL,
    values_json TEXT NOT NULL,
    asset_ids_json TEXT NOT NULL,
    production_batch_item_id TEXT,
    task_id TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (experiment_id) REFERENCES benchmark_experiments(id) ON DELETE CASCADE,
    FOREIGN KEY (workflow_version_id) REFERENCES workflow_versions(id),
    FOREIGN KEY (recipe_id) REFERENCES recipes(id),
    FOREIGN KEY (production_batch_item_id) REFERENCES production_batch_items(id) ON DELETE SET NULL,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL,
    UNIQUE(experiment_id, position)
);

CREATE INDEX idx_benchmark_candidates_workflow
    ON benchmark_candidates(workflow_version_id, recipe_id);

CREATE INDEX idx_benchmark_candidates_batch_item
    ON benchmark_candidates(production_batch_item_id);
