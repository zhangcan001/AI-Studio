-- Submission identity is a first-class, project-scoped database invariant.
-- Existing tasks remain readable: legacy rows keep NULL identity metadata and
-- are treated as the original attempt by the domain layer.
ALTER TABLE tasks ADD COLUMN submission_idempotency_key TEXT;
ALTER TABLE tasks ADD COLUMN submission_attempt INTEGER;
ALTER TABLE tasks ADD COLUMN parent_task_id TEXT;

-- Preserve keys written by DEV-019 before they became indexed task columns.
UPDATE tasks
SET submission_idempotency_key = (
        SELECT json_extract(e.payload_json, '$.submissionIdempotencyKey')
        FROM task_events e
        WHERE e.task_id = tasks.id
          AND e.event_type = 'TASK_SUBMISSION_PREPARED'
          AND e.payload_json IS NOT NULL
          AND json_extract(e.payload_json, '$.submissionIdempotencyKey') IS NOT NULL
        ORDER BY e.sequence ASC
        LIMIT 1
    ),
    submission_attempt = COALESCE(submission_attempt, 1)
WHERE submission_idempotency_key IS NULL
  AND EXISTS (
        SELECT 1
        FROM task_events e
        WHERE e.task_id = tasks.id
          AND e.event_type = 'TASK_SUBMISSION_PREPARED'
          AND e.payload_json IS NOT NULL
          AND json_extract(e.payload_json, '$.submissionIdempotencyKey') IS NOT NULL
    );

CREATE UNIQUE INDEX idx_tasks_project_submission_idempotency
    ON tasks(project_id, submission_idempotency_key)
    WHERE submission_idempotency_key IS NOT NULL;

CREATE INDEX idx_tasks_parent_task_id
    ON tasks(parent_task_id)
    WHERE parent_task_id IS NOT NULL;
