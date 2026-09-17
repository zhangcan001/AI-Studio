-- Artifact records reuse the existing generated Asset and task_output_assets
-- authorities. Reviews are now scoped to one concrete generated asset.
CREATE TABLE artifact_reviews (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    artifact_id TEXT NOT NULL UNIQUE,
    decision TEXT NOT NULL DEFAULT 'PENDING'
        CHECK (decision IN ('PENDING', 'APPROVED', 'REJECTED')),
    comment TEXT NOT NULL DEFAULT '',
    revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (artifact_id) REFERENCES assets(id) ON DELETE CASCADE
);

CREATE INDEX idx_artifact_reviews_project_decision
    ON artifact_reviews(project_id, decision, updated_at DESC, artifact_id);

-- Migrate only exact historical artifact links. Batch-item decisions without a
-- result_asset_id cannot be safely assigned to one of a task's multiple files.
INSERT INTO artifact_reviews
    (id, project_id, artifact_id, decision, comment, revision, created_at, updated_at)
SELECT
    'arv_' || lower(hex(randomblob(16))),
    a.project_id,
    a.id,
    CASE legacy.review_status
        WHEN 'APPROVED' THEN 'APPROVED'
        WHEN 'REJECTED' THEN 'REJECTED'
        ELSE 'PENDING'
    END,
    CASE WHEN legacy.review_status IN ('APPROVED', 'REJECTED') THEN legacy.review_note ELSE '' END,
    0,
    toa.created_at,
    COALESCE(legacy.updated_at, toa.created_at)
FROM task_output_assets toa
INNER JOIN assets a ON a.id = toa.asset_id
INNER JOIN tasks t ON t.id = toa.task_id AND t.project_id = a.project_id
LEFT JOIN production_item_reviews legacy
    ON legacy.id = (
       SELECT review.id
       FROM production_item_reviews review
       WHERE review.project_id = a.project_id
         AND review.result_asset_id = a.id
       ORDER BY review.updated_at DESC, review.id DESC
       LIMIT 1
   );

-- Every newly registered task output gets its own pending artifact review in
-- the same SQLite transaction as the output mapping insert.
CREATE TRIGGER task_output_assets_create_artifact_review
AFTER INSERT ON task_output_assets
BEGIN
    INSERT OR IGNORE INTO artifact_reviews
        (id, project_id, artifact_id, decision, comment, revision, created_at, updated_at)
    SELECT
        'arv_' || lower(hex(randomblob(16))),
        assets.project_id,
        assets.id,
        'PENDING',
        '',
        0,
        NEW.created_at,
        NEW.created_at
    FROM assets
    WHERE assets.id = NEW.asset_id;
END;
