CREATE TABLE asset_video_prompts (
    asset_id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    prompt_text TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE CASCADE,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX idx_asset_video_prompts_project_updated
    ON asset_video_prompts(project_id, updated_at DESC, asset_id ASC);
