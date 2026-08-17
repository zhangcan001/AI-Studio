-- A Shot's narrative description is not a sufficient production prompt:
-- image composition and video motion need independent, immutable snapshots.
CREATE TABLE shot_stage_prompts (
    shot_id TEXT NOT NULL,
    stage TEXT NOT NULL CHECK (stage IN ('image', 'video')),
    prompt_text TEXT NOT NULL,
    prompt_entry_id TEXT,
    prompt_version_id TEXT,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (shot_id, stage),
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE,
    FOREIGN KEY (prompt_entry_id) REFERENCES prompt_entries(id) ON DELETE SET NULL,
    FOREIGN KEY (prompt_version_id) REFERENCES prompt_versions(id) ON DELETE SET NULL
);

CREATE INDEX idx_shot_stage_prompts_shot
ON shot_stage_prompts(shot_id, stage);

-- Preserve the pre-019 contract for existing projects while allowing future
-- imports and assignments to diverge by stage.
INSERT INTO shot_stage_prompts (
    shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at
)
SELECT id, 'image', prompt_text, prompt_entry_id, prompt_version_id, updated_at
FROM shots;

INSERT INTO shot_stage_prompts (
    shot_id, stage, prompt_text, prompt_entry_id, prompt_version_id, updated_at
)
SELECT id, 'video', prompt_text, prompt_entry_id, prompt_version_id, updated_at
FROM shots;
