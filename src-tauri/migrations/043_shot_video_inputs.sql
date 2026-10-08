-- Reviewed necessity/model: docs/architecture/minimax-video-phase2-design.md.
-- Additive only: legacy result/reference rows are not inferred or migrated.
CREATE UNIQUE INDEX idx_shots_project_identity ON shots(project_id, id);
CREATE UNIQUE INDEX idx_assets_project_identity ON assets(project_id, id);

CREATE TABLE shot_video_input_sets (
    project_id TEXT NOT NULL,
    shot_id TEXT NOT NULL,
    workflow_version_id TEXT NOT NULL CHECK (length(trim(workflow_version_id)) > 0),
    recipe_id TEXT NOT NULL CHECK (length(trim(recipe_id)) > 0),
    instance_id TEXT NOT NULL CHECK (length(trim(instance_id)) > 0),
    revision INTEGER NOT NULL CHECK (revision > 0),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (project_id, shot_id, workflow_version_id, recipe_id),
    FOREIGN KEY (project_id, shot_id) REFERENCES shots(project_id, id) ON DELETE CASCADE
);

CREATE TABLE shot_video_input_assets (
    project_id TEXT NOT NULL,
    shot_id TEXT NOT NULL,
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    input_key TEXT NOT NULL CHECK (input_key IN (
        'first_frame', 'last_frame', 'reference_images', 'reference_videos', 'reference_audios'
    )),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    asset_id TEXT NOT NULL,
    CHECK (input_key NOT IN ('first_frame', 'last_frame') OR ordinal = 0),
    PRIMARY KEY (project_id, shot_id, workflow_version_id, recipe_id, input_key, ordinal),
    FOREIGN KEY (project_id, shot_id, workflow_version_id, recipe_id)
        REFERENCES shot_video_input_sets(project_id, shot_id, workflow_version_id, recipe_id)
        ON DELETE CASCADE,
    FOREIGN KEY (project_id, asset_id) REFERENCES assets(project_id, id)
        ON DELETE NO ACTION DEFERRABLE INITIALLY DEFERRED
);
CREATE INDEX idx_shot_video_input_assets_asset ON shot_video_input_assets(project_id, asset_id);

-- Only the source import repository write creates this receipt, atomically with
-- the Asset. Generic Asset/category/metadata writes do not attest provenance.
CREATE TABLE external_asset_imports (
    asset_id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    media_type TEXT NOT NULL CHECK (media_type IN ('image', 'video', 'audio')),
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64 AND sha256 NOT GLOB '*[^0-9a-f]*'),
    mime_type TEXT NOT NULL CHECK (length(trim(mime_type)) > 0),
    file_size INTEGER NOT NULL CHECK (file_size > 0),
    width INTEGER NOT NULL CHECK (width >= 0),
    height INTEGER NOT NULL CHECK (height >= 0),
    duration_ms INTEGER CHECK (duration_ms > 0),
    imported_at TEXT NOT NULL,
    FOREIGN KEY (project_id, asset_id) REFERENCES assets(project_id, id) ON DELETE CASCADE
);
