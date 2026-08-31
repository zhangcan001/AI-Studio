-- Durable source provenance for Production Package queue batches.
-- This is tracing/grouping metadata only; queue, task, and asset state stays
-- in its existing tables.
CREATE TABLE production_package_batch_bindings (
    project_id TEXT NOT NULL,
    package_key TEXT NOT NULL
        CHECK (length(package_key) = 64 AND package_key NOT GLOB '*[^0-9a-f]*'),
    package_root TEXT NOT NULL,
    manifest_sha256 TEXT NOT NULL
        CHECK (length(manifest_sha256) = 64 AND manifest_sha256 NOT GLOB '*[^0-9a-f]*'),
    package_id TEXT,
    package_name TEXT NOT NULL,
    batch_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL CHECK (chunk_index >= 0),
    chunk_count INTEGER NOT NULL CHECK (chunk_count > 0),
    package_item_ids_json TEXT NOT NULL CHECK (json_valid(package_item_ids_json)),
    created_at TEXT NOT NULL,
    source_kind TEXT NOT NULL DEFAULT 'PRODUCTION_PACKAGE'
        CHECK (source_kind = 'PRODUCTION_PACKAGE'),

    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (batch_id) REFERENCES production_batches(id) ON DELETE CASCADE,
    UNIQUE (project_id, package_key, batch_id),
    CHECK (chunk_index < chunk_count)
);

CREATE INDEX idx_production_package_batch_bindings_project_package
    ON production_package_batch_bindings(project_id, package_key);
