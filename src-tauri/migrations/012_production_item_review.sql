CREATE TABLE production_item_reviews (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    production_batch_id TEXT NOT NULL,
    production_batch_item_id TEXT NOT NULL UNIQUE,
    task_id TEXT,
    result_asset_id TEXT,
    review_status TEXT NOT NULL DEFAULT 'UNREVIEWED'
        CHECK (review_status IN ('UNREVIEWED', 'APPROVED', 'STARRED', 'REGENERATE', 'REJECTED')),
    review_note TEXT NOT NULL DEFAULT '',
    version INTEGER NOT NULL CHECK (version >= 1),
    lineage_key TEXT NOT NULL,
    parent_batch_id TEXT,
    parent_item_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (production_batch_id) REFERENCES production_batches(id) ON DELETE CASCADE,
    FOREIGN KEY (production_batch_item_id) REFERENCES production_batch_items(id) ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL,
    FOREIGN KEY (result_asset_id) REFERENCES assets(id) ON DELETE SET NULL,
    FOREIGN KEY (parent_batch_id) REFERENCES production_batches(id) ON DELETE SET NULL,
    FOREIGN KEY (parent_item_id) REFERENCES production_batch_items(id) ON DELETE SET NULL
);

CREATE INDEX idx_production_item_reviews_batch
    ON production_item_reviews(project_id, production_batch_id, version, production_batch_item_id);

CREATE INDEX idx_production_item_reviews_lineage
    ON production_item_reviews(project_id, lineage_key, version);
