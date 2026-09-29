-- Add optimistic-concurrency identity to project workflow bindings.
-- The instance id is deliberately backfilled per row so deleting and
-- recreating a slot cannot make an old client token valid again.
ALTER TABLE project_workflow_bindings
    ADD COLUMN revision INTEGER NOT NULL DEFAULT 1;

ALTER TABLE project_workflow_bindings
    ADD COLUMN binding_instance_id TEXT;

UPDATE project_workflow_bindings
SET binding_instance_id = 'bnd_' || lower(hex(randomblob(16)))
WHERE binding_instance_id IS NULL OR trim(binding_instance_id) = '';
