-- User-deleted task records are hidden, never physically removed, so lineage
-- (assets.source_task_id, task_events, batch items) stays intact.
-- ADD COLUMN with NULL default: no table rebuild, no existing row rewritten.
ALTER TABLE tasks ADD COLUMN hidden_at TEXT;
