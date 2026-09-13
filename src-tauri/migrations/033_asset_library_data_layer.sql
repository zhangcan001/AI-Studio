-- Asset Library data foundation. Existing asset/task/reference authorities remain
-- authoritative; these tables add only immutable version history and generic
-- project-local asset relations.
CREATE TABLE asset_versions (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    version_number INTEGER NOT NULL CHECK (version_number >= 1),
    metadata_snapshot TEXT NOT NULL,
    location TEXT NOT NULL CHECK (length(trim(location)) > 0),
    checksum TEXT NOT NULL CHECK (length(trim(checksum)) > 0),
    created_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE,
    UNIQUE (project_id, asset_id, version_number)
);

CREATE INDEX idx_asset_versions_project_asset_version
    ON asset_versions(project_id, asset_id, version_number DESC, id ASC);

CREATE TABLE asset_relations (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    source_asset_id TEXT NOT NULL,
    target_asset_id TEXT NOT NULL,
    relation_type TEXT NOT NULL CHECK (
        relation_type IN (
            'SOURCE_OF', 'DERIVED_FROM', 'VARIANT_OF',
            'REFERENCE', 'REPLACEMENT', 'RELATED'
        )
    ),
    created_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (source_asset_id) REFERENCES assets(id) ON DELETE CASCADE,
    FOREIGN KEY (target_asset_id) REFERENCES assets(id) ON DELETE CASCADE,
    CHECK (source_asset_id <> target_asset_id),
    UNIQUE (project_id, source_asset_id, target_asset_id, relation_type)
);

CREATE INDEX idx_asset_relations_project_source
    ON asset_relations(project_id, source_asset_id, created_at ASC, id ASC);

CREATE INDEX idx_asset_relations_project_target
    ON asset_relations(project_id, target_asset_id, created_at ASC, id ASC);
