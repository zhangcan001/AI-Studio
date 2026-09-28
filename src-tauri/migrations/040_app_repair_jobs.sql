-- One row per application-level data repair job (for example re-publishing
-- recipes whose defaults were truncated by an older recognizer). Recipes are
-- immutable, so repairs publish new recipe versions; this table only records
-- whether a job already completed so it runs at most once per database.
CREATE TABLE IF NOT EXISTS app_repair_jobs (
    job_id TEXT PRIMARY KEY,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    status TEXT NOT NULL CHECK (status IN ('RUNNING', 'COMPLETED', 'FAILED', 'SKIPPED')),
    summary_json TEXT NOT NULL DEFAULT '{}'
);
