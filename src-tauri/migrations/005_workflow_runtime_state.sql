CREATE TABLE workflow_runtime_states (
    workflow_version_id TEXT PRIMARY KEY,

    enabled INTEGER NOT NULL
        CHECK(enabled IN (0, 1)),

    updated_at TEXT NOT NULL,

    FOREIGN KEY (workflow_version_id)
        REFERENCES workflow_versions(id)
);

CREATE INDEX idx_workflow_runtime_states_enabled
ON workflow_runtime_states(enabled);
