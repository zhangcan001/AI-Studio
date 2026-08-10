CREATE TABLE shots (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    name TEXT NOT NULL,
    prompt_text TEXT NOT NULL,
    prompt_entry_id TEXT,
    prompt_version_id TEXT,
    selected_image_asset_id TEXT,
    selected_video_asset_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (prompt_entry_id) REFERENCES prompt_entries(id) ON DELETE SET NULL,
    FOREIGN KEY (prompt_version_id) REFERENCES prompt_versions(id) ON DELETE SET NULL,
    FOREIGN KEY (selected_image_asset_id) REFERENCES assets(id) ON DELETE SET NULL,
    FOREIGN KEY (selected_video_asset_id) REFERENCES assets(id) ON DELETE SET NULL,
    UNIQUE(project_id, ordinal)
);

CREATE INDEX idx_shots_project_ordinal
ON shots(project_id, ordinal ASC, id ASC);

CREATE TABLE shot_stage_configs (
    shot_id TEXT NOT NULL,
    stage TEXT NOT NULL CHECK (stage IN ('image', 'video')),
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    scalar_values_json TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (shot_id, stage),
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE,
    FOREIGN KEY (workflow_version_id) REFERENCES workflow_versions(id),
    FOREIGN KEY (recipe_id) REFERENCES recipes(id)
);

CREATE TABLE shot_reference_assets (
    shot_id TEXT NOT NULL,
    stage TEXT NOT NULL CHECK (stage IN ('image', 'video')),
    asset_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY (shot_id, stage, asset_id),
    UNIQUE (shot_id, stage, ordinal),
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE INDEX idx_shot_reference_assets_stage
ON shot_reference_assets(shot_id, stage, ordinal ASC, asset_id ASC);

CREATE TABLE shot_generation_links (
    id TEXT PRIMARY KEY,
    shot_id TEXT NOT NULL,
    stage TEXT NOT NULL CHECK (stage IN ('image', 'video')),
    task_id TEXT,
    production_batch_item_id TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL,
    FOREIGN KEY (production_batch_item_id) REFERENCES production_batch_items(id) ON DELETE SET NULL,
    UNIQUE(task_id)
);

CREATE INDEX idx_shot_generation_links_shot_stage
ON shot_generation_links(shot_id, stage, created_at DESC, id DESC);
