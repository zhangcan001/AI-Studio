ALTER TABLE production_batches ADD COLUMN archived_at TEXT;
ALTER TABLE production_batch_items ADD COLUMN retry_of_item_id TEXT;

CREATE INDEX idx_production_batches_project_archived_updated
    ON production_batches(project_id, archived_at, updated_at DESC, id ASC);

CREATE INDEX idx_production_batch_items_retry_of
    ON production_batch_items(retry_of_item_id);
