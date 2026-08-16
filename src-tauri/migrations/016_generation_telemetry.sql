-- Generation telemetry is diagnostic metadata. All columns are nullable so
-- tasks created before this migration remain valid and readable.
ALTER TABLE tasks ADD COLUMN generation_execution_id TEXT;
ALTER TABLE tasks ADD COLUMN compiled_workflow_sha256 TEXT;
ALTER TABLE tasks ADD COLUMN runtime_profile TEXT;
ALTER TABLE tasks ADD COLUMN concurrency_class TEXT;
ALTER TABLE tasks ADD COLUMN prepare_started_at TEXT;
ALTER TABLE tasks ADD COLUMN prepared_at TEXT;
ALTER TABLE tasks ADD COLUMN submitted_at TEXT;
ALTER TABLE tasks ADD COLUMN execution_started_at TEXT;
ALTER TABLE tasks ADD COLUMN execution_finished_at TEXT;
ALTER TABLE tasks ADD COLUMN collection_finished_at TEXT;
