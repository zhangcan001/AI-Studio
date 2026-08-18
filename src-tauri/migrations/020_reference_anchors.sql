CREATE TABLE reference_anchors (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('CHARACTER', 'SCENE', 'PROP', 'STYLE')),
    name TEXT NOT NULL,
    normalized_name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    UNIQUE (project_id, kind, normalized_name)
);

CREATE TABLE reference_anchor_assets (
    anchor_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    created_at TEXT NOT NULL,
    PRIMARY KEY (anchor_id, asset_id),
    UNIQUE (anchor_id, ordinal),
    FOREIGN KEY (anchor_id) REFERENCES reference_anchors(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE INDEX idx_reference_anchors_project_kind
ON reference_anchors(project_id, kind);

CREATE INDEX idx_reference_anchor_assets_anchor_ordinal
ON reference_anchor_assets(anchor_id, ordinal);
