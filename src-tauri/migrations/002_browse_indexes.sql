CREATE INDEX IF NOT EXISTS idx_tasks_project_created
ON tasks(project_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_tasks_project_status_created
ON tasks(project_id, status, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_assets_project_created
ON assets(project_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_assets_project_category_created
ON assets(project_id, category, created_at DESC, id DESC);
