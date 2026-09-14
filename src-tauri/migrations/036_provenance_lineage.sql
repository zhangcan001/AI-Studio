-- Provenance lineage is additive. The existing Task, output mapping, and
-- AssetVersion authorities remain the source of truth; these tables only add
-- explicit edges between them and the canonical local Tool registry.
CREATE TABLE generation_tool_usages (
    id TEXT PRIMARY KEY NOT NULL,
    generation_id TEXT NOT NULL,
    tool_instance_id TEXT NOT NULL,
    tool_version_id TEXT,
    metadata_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (generation_id) REFERENCES tasks(id) ON DELETE RESTRICT,
    FOREIGN KEY (tool_instance_id) REFERENCES tool_instances(id) ON DELETE RESTRICT,
    FOREIGN KEY (tool_version_id) REFERENCES tool_versions(id) ON DELETE RESTRICT
);

CREATE INDEX idx_generation_tool_usages_generation_created
    ON generation_tool_usages(generation_id, created_at ASC, id ASC);

CREATE INDEX idx_generation_tool_usages_tool_instance_created
    ON generation_tool_usages(tool_instance_id, created_at ASC, id ASC);

CREATE TABLE generation_asset_versions (
    id TEXT PRIMARY KEY NOT NULL,
    generation_id TEXT NOT NULL,
    output_id TEXT NOT NULL CHECK (length(trim(output_id)) > 0),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    asset_version_id TEXT NOT NULL,
    relation_type TEXT NOT NULL CHECK (
        relation_type IN ('OUTPUT', 'DERIVED')
    ),
    created_at TEXT NOT NULL,
    FOREIGN KEY (generation_id, output_id, ordinal)
        REFERENCES task_output_assets(task_id, output_id, ordinal)
        ON DELETE RESTRICT,
    FOREIGN KEY (asset_version_id) REFERENCES asset_versions(id) ON DELETE RESTRICT,
    UNIQUE (generation_id, output_id, ordinal)
);

CREATE INDEX idx_generation_asset_versions_generation_created
    ON generation_asset_versions(generation_id, created_at ASC, id ASC);

CREATE INDEX idx_generation_asset_versions_asset_version
    ON generation_asset_versions(asset_version_id, created_at ASC, id ASC);
