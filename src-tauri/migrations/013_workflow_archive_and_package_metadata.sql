ALTER TABLE workflow_versions ADD COLUMN package_name TEXT;

ALTER TABLE workflow_runtime_states ADD COLUMN archived INTEGER NOT NULL DEFAULT 0
    CHECK(archived IN (0, 1));

ALTER TABLE workflow_runtime_states ADD COLUMN archived_at TEXT;

CREATE INDEX idx_workflow_runtime_states_archived
ON workflow_runtime_states(archived);
