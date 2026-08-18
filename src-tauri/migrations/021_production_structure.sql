CREATE TABLE production_series (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    UNIQUE (project_id, ordinal)
);

CREATE TABLE production_episodes (
    id TEXT PRIMARY KEY,
    series_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (series_id) REFERENCES production_series(id) ON DELETE CASCADE,
    UNIQUE (series_id, ordinal)
);

CREATE TABLE production_scenes (
    id TEXT PRIMARY KEY,
    episode_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (episode_id) REFERENCES production_episodes(id) ON DELETE CASCADE,
    UNIQUE (episode_id, ordinal)
);

CREATE TABLE shot_scene_assignments (
    shot_id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE,
    FOREIGN KEY (scene_id) REFERENCES production_scenes(id) ON DELETE CASCADE,
    UNIQUE (scene_id, ordinal)
);

CREATE INDEX idx_production_series_project_ordinal
ON production_series(project_id, ordinal);

CREATE INDEX idx_production_episodes_series_ordinal
ON production_episodes(series_id, ordinal);

CREATE INDEX idx_production_scenes_episode_ordinal
ON production_scenes(episode_id, ordinal);

CREATE INDEX idx_shot_scene_assignments_scene_ordinal
ON shot_scene_assignments(scene_id, ordinal);
