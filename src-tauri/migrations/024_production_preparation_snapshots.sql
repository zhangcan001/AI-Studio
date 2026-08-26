CREATE TABLE production_preparation_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    shot_id TEXT NOT NULL,
    stage TEXT NOT NULL CHECK (stage IN ('image', 'video')),
    context_hash TEXT NOT NULL,
    production_batch_id TEXT NOT NULL,
    production_batch_item_id TEXT NOT NULL UNIQUE,
    snapshot_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE,
    FOREIGN KEY (production_batch_id) REFERENCES production_batches(id) ON DELETE CASCADE,
    FOREIGN KEY (production_batch_item_id) REFERENCES production_batch_items(id) ON DELETE CASCADE
);

CREATE INDEX idx_production_preparation_snapshots_project_shot_context
    ON production_preparation_snapshots(project_id, shot_id, stage, context_hash);

CREATE INDEX idx_production_preparation_snapshots_batch
    ON production_preparation_snapshots(production_batch_id);

CREATE INDEX idx_production_preparation_snapshots_batch_item
    ON production_preparation_snapshots(production_batch_item_id);
