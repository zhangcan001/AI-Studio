ALTER TABLE assets ADD COLUMN duration_ms INTEGER;

CREATE TABLE task_output_assets (
    task_id TEXT NOT NULL,
    output_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    asset_id TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL,

    PRIMARY KEY (task_id, output_id, ordinal),

    FOREIGN KEY (task_id)
        REFERENCES tasks(id),

    FOREIGN KEY (asset_id)
        REFERENCES assets(id)
);

CREATE INDEX idx_task_output_assets_task
ON task_output_assets(task_id, output_id, ordinal);
