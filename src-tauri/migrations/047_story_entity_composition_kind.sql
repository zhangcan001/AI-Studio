-- Adds the 'composition' story entity kind. SQLite cannot alter a CHECK, so the
-- table is rebuilt. sqlx runs SQLite migrations inside a transaction, where
-- PRAGMA foreign_keys cannot be switched off; dropping the old table therefore
-- cascades into story_entity_images and shot_story_entity_bindings. Both child
-- tables are snapshotted first and restored verbatim after the rename, so no
-- image link or shot binding is lost. Children keep referencing story_entities.
CREATE TEMP TABLE story_entity_images_047 AS SELECT * FROM story_entity_images;
CREATE TEMP TABLE shot_story_entity_bindings_047 AS SELECT * FROM shot_story_entity_bindings;

CREATE TABLE story_entities_new (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('character', 'scene', 'prop', 'composition')),
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    cover_asset_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (cover_asset_id) REFERENCES assets(id) ON DELETE SET NULL
);

INSERT INTO story_entities_new (id, project_id, kind, name, description, cover_asset_id, created_at, updated_at)
SELECT id, project_id, kind, name, description, cover_asset_id, created_at, updated_at FROM story_entities;

DROP TABLE story_entities;

ALTER TABLE story_entities_new RENAME TO story_entities;

CREATE UNIQUE INDEX uq_story_entities_project_kind_name
    ON story_entities(project_id, kind, name COLLATE NOCASE);

CREATE INDEX idx_story_entities_project_kind_updated
    ON story_entities(project_id, kind, updated_at, id);

INSERT INTO story_entity_images (entity_id, asset_id, ordinal, created_at)
SELECT entity_id, asset_id, ordinal, created_at FROM story_entity_images_047;

INSERT INTO shot_story_entity_bindings (id, project_id, shot_id, entity_id, ordinal, created_at, updated_at)
SELECT id, project_id, shot_id, entity_id, ordinal, created_at, updated_at FROM shot_story_entity_bindings_047;

DROP TABLE story_entity_images_047;
DROP TABLE shot_story_entity_bindings_047;
