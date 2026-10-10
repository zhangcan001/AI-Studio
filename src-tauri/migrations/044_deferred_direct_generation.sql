-- Phase 4 deferred direct generation admission. The queue runner remains the
-- sole execution authority; this table persists *intent to start*, not Tasks.
CREATE TABLE production_deferred_starts (
    batch_id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'WAITING'
        CHECK(state IN ('WAITING', 'BLOCKED')),
    last_error TEXT,
    requested_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (batch_id) REFERENCES production_batches(id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);
CREATE INDEX idx_production_deferred_starts_fifo
    ON production_deferred_starts(state, requested_at ASC, batch_id ASC);
