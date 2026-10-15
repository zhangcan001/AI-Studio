-- Additive project-scoped story entities for storyboard chips (Character/Scene/Prop).
-- Does not alter existing consistency profile tables or backup v21 profile column lists.
CREATE TABLE story_entities (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('character', 'scene', 'prop')),
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    cover_asset_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (cover_asset_id) REFERENCES assets(id) ON DELETE SET NULL
);

CREATE UNIQUE INDEX uq_story_entities_project_kind_name
    ON story_entities(project_id, kind, name COLLATE NOCASE);

CREATE INDEX idx_story_entities_project_kind_updated
    ON story_entities(project_id, kind, updated_at, id);

CREATE TABLE story_entity_images (
    entity_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    created_at TEXT NOT NULL,
    PRIMARY KEY (entity_id, asset_id),
    UNIQUE (entity_id, ordinal),
    FOREIGN KEY (entity_id) REFERENCES story_entities(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE INDEX idx_story_entity_images_asset
    ON story_entity_images(asset_id);
