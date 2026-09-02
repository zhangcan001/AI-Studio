-- Durable project-level workflow defaults and mode overrides.
-- Workflow and recipe references intentionally remain soft so stale bindings
-- survive workflow removal and can be surfaced to the user for repair.
CREATE TABLE project_workflow_bindings (
    project_id TEXT NOT NULL,
    stage TEXT NOT NULL,
    mode TEXT NOT NULL,
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,

    PRIMARY KEY (project_id, stage, mode),
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    CHECK (stage IN ('IMAGE', 'VIDEO')),
    CHECK (mode IN (
        'DEFAULT',
        'FL2VA_TEXT_TO_VIDEO',
        'FL2VA_IMAGE_TO_VIDEO',
        'FL2VA_FIRST_LAST',
        'REF2VA_IMAGE',
        'REF2VA_AUDIO',
        'REF2VA_IMAGE_AUDIO',
        'REF2VA_VIDEO_IMAGE'
    )),
    CHECK (
        (stage = 'IMAGE' AND mode = 'DEFAULT') OR
        (stage = 'VIDEO' AND mode IN (
            'DEFAULT',
            'FL2VA_TEXT_TO_VIDEO',
            'FL2VA_IMAGE_TO_VIDEO',
            'FL2VA_FIRST_LAST',
            'REF2VA_IMAGE',
            'REF2VA_AUDIO',
            'REF2VA_IMAGE_AUDIO',
            'REF2VA_VIDEO_IMAGE'
        ))
    )
);

CREATE INDEX idx_project_workflow_bindings_project
    ON project_workflow_bindings(project_id);
