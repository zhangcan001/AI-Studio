-- Shot bindings to project story entities (Character/Scene/Prop) for storyboard chips.
CREATE TABLE shot_story_entity_bindings (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    shot_id TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL DEFAULT 0 CHECK (ordinal >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE,
    FOREIGN KEY (entity_id) REFERENCES story_entities(id) ON DELETE CASCADE,
    UNIQUE (shot_id, entity_id),
    UNIQUE (shot_id, ordinal)
);

CREATE INDEX idx_shot_story_entity_bindings_project_shot
    ON shot_story_entity_bindings(project_id, shot_id, ordinal);
