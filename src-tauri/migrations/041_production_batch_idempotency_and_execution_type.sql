-- Formalize direct-submission identity without changing legacy JSON payloads.
-- project_id is denormalized on the item only to make the uniqueness scope
-- explicit in SQLite; the value is backfilled from the authoritative batch.
ALTER TABLE production_batch_items ADD COLUMN project_id TEXT;
ALTER TABLE production_batch_items ADD COLUMN submission_idempotency_key TEXT;
ALTER TABLE production_batch_items ADD COLUMN execution_type TEXT;

UPDATE production_batch_items
SET project_id = (
    SELECT project_id
    FROM production_batches
    WHERE production_batches.id = production_batch_items.batch_id
)
WHERE project_id IS NULL;

CREATE INDEX idx_production_batch_items_project_submission_key
    ON production_batch_items(project_id, submission_idempotency_key)
    WHERE submission_idempotency_key IS NOT NULL;

CREATE UNIQUE INDEX uq_production_batch_items_project_submission_key
    ON production_batch_items(project_id, submission_idempotency_key)
    WHERE submission_idempotency_key IS NOT NULL;
