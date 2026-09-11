CREATE TABLE workflow_recipe_runtime_states (
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    archived INTEGER NOT NULL DEFAULT 0 CHECK (archived IN (0, 1)),
    archived_at TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (workflow_version_id, recipe_id),
    FOREIGN KEY (workflow_version_id, recipe_id)
        REFERENCES recipes(workflow_version_id, id)
        ON DELETE CASCADE
);

CREATE INDEX idx_workflow_recipe_runtime_states_updated_at
    ON workflow_recipe_runtime_states(updated_at DESC);
