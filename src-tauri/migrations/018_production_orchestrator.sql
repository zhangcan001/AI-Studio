-- Benchmark 2.0 experiment controls and frozen candidate identity.
ALTER TABLE benchmark_experiments ADD COLUMN seed_strategy TEXT NOT NULL DEFAULT 'FIXED_SEED';
ALTER TABLE benchmark_experiments ADD COLUMN fixed_seed TEXT;
ALTER TABLE benchmark_experiments ADD COLUMN repeat_count INTEGER NOT NULL DEFAULT 3;
ALTER TABLE benchmark_experiments ADD COLUMN recommendation_type TEXT;

ALTER TABLE benchmark_candidates ADD COLUMN workflow_id TEXT;
ALTER TABLE benchmark_candidates ADD COLUMN workflow_version TEXT;
ALTER TABLE benchmark_candidates ADD COLUMN workflow_sha256 TEXT;
ALTER TABLE benchmark_candidates ADD COLUMN recipe_version TEXT;
ALTER TABLE benchmark_candidates ADD COLUMN recipe_sha256 TEXT;
ALTER TABLE benchmark_candidates ADD COLUMN runtime_package TEXT;
ALTER TABLE benchmark_candidates ADD COLUMN runtime_profile TEXT;

CREATE TABLE benchmark_runs (
    id TEXT PRIMARY KEY NOT NULL,
    experiment_id TEXT NOT NULL,
    candidate_id TEXT NOT NULL,
    run_number INTEGER NOT NULL CHECK (run_number >= 1),
    production_batch_item_id TEXT,
    task_id TEXT,
    snapshot_id TEXT,
    output_asset_id TEXT,
    generation_execution_id TEXT,
    compiled_workflow_sha256 TEXT,
    runtime_profile TEXT,
    concurrency_class TEXT,
    queue_wait_ms INTEGER,
    prepare_ms INTEGER,
    submit_ms INTEGER,
    comfy_execution_ms INTEGER,
    collect_ms INTEGER,
    total_ms INTEGER,
    status TEXT,
    error_code TEXT,
    output_file_size INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (experiment_id) REFERENCES benchmark_experiments(id) ON DELETE CASCADE,
    FOREIGN KEY (candidate_id) REFERENCES benchmark_candidates(id) ON DELETE CASCADE,
    FOREIGN KEY (production_batch_item_id) REFERENCES production_batch_items(id) ON DELETE SET NULL,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL,
    FOREIGN KEY (snapshot_id) REFERENCES generation_snapshots(id) ON DELETE SET NULL,
    FOREIGN KEY (output_asset_id) REFERENCES assets(id) ON DELETE SET NULL,
    UNIQUE(candidate_id, run_number)
);

CREATE INDEX idx_benchmark_runs_experiment_candidate
    ON benchmark_runs(experiment_id, candidate_id, run_number);

CREATE INDEX idx_benchmark_runs_task
    ON benchmark_runs(task_id);

CREATE TABLE benchmark_quality_scores (
    id TEXT PRIMARY KEY NOT NULL,
    candidate_id TEXT NOT NULL,
    prompt_adherence INTEGER CHECK (prompt_adherence BETWEEN 1 AND 5),
    visual_quality INTEGER CHECK (visual_quality BETWEEN 1 AND 5),
    motion_quality INTEGER CHECK (motion_quality BETWEEN 1 AND 5),
    reference_consistency INTEGER CHECK (reference_consistency BETWEEN 1 AND 5),
    overall INTEGER CHECK (overall BETWEEN 1 AND 5),
    note TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (candidate_id) REFERENCES benchmark_candidates(id) ON DELETE CASCADE,
    UNIQUE(candidate_id)
);

CREATE INDEX idx_benchmark_quality_candidate
    ON benchmark_quality_scores(candidate_id);

-- Production Orchestrator foundation. The orchestrator references normal
-- batches/tasks/assets and never stores a duplicate workflow JSON.
CREATE TABLE production_runs (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN (
            'DRAFT', 'READY', 'RUNNING', 'WAITING_FOR_SELECTION',
            'SUCCEEDED', 'PARTIAL_FAILED', 'FAILED', 'CANCELLED'
        )
    ),
    current_stage_ordinal INTEGER NOT NULL DEFAULT 0,
    template_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE INDEX idx_production_runs_project_updated
    ON production_runs(project_id, updated_at DESC, id ASC);

CREATE INDEX idx_production_runs_project_status
    ON production_runs(project_id, status);

CREATE TABLE production_stages (
    id TEXT PRIMARY KEY NOT NULL,
    run_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    stage_type TEXT NOT NULL CHECK (
        stage_type IN (
            'KREA2_IMAGE_GENERATION',
            'ASSET_SELECTION',
            'H3_VIDEO_GENERATION'
        )
    ),
    status TEXT NOT NULL CHECK (
        status IN (
            'PENDING', 'READY', 'RUNNING', 'WAITING',
            'SUCCEEDED', 'FAILED', 'SKIPPED', 'CANCELLED'
        )
    ),
    workflow_version_id TEXT,
    recipe_id TEXT,
    production_batch_id TEXT,
    frozen_config_json TEXT NOT NULL,
    prompt TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    started_at TEXT,
    finished_at TEXT,
    FOREIGN KEY (run_id) REFERENCES production_runs(id) ON DELETE CASCADE,
    FOREIGN KEY (workflow_version_id) REFERENCES workflow_versions(id) ON DELETE SET NULL,
    FOREIGN KEY (recipe_id) REFERENCES recipes(id) ON DELETE SET NULL,
    FOREIGN KEY (production_batch_id) REFERENCES production_batches(id) ON DELETE SET NULL,
    UNIQUE(run_id, ordinal)
);

CREATE INDEX idx_production_stages_run_status
    ON production_stages(run_id, status, ordinal);

CREATE INDEX idx_production_stages_batch
    ON production_stages(production_batch_id);

CREATE TABLE production_stage_items (
    id TEXT PRIMARY KEY NOT NULL,
    stage_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    status TEXT NOT NULL CHECK (
        status IN (
            'PENDING', 'READY', 'RUNNING', 'WAITING',
            'SUCCEEDED', 'FAILED', 'SKIPPED', 'CANCELLED'
        )
    ),
    production_batch_item_id TEXT,
    task_id TEXT,
    asset_id TEXT,
    source_asset_id TEXT,
    reference_index INTEGER,
    attempt INTEGER NOT NULL DEFAULT 1 CHECK (attempt >= 1),
    submission_idempotency_key TEXT,
    parent_stage_item_id TEXT,
    frozen_values_json TEXT NOT NULL,
    error_code TEXT,
    error_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (stage_id) REFERENCES production_stages(id) ON DELETE CASCADE,
    FOREIGN KEY (production_batch_item_id) REFERENCES production_batch_items(id) ON DELETE SET NULL,
    FOREIGN KEY (task_id) REFERENCES tasks(id) ON DELETE SET NULL,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE SET NULL,
    FOREIGN KEY (source_asset_id) REFERENCES assets(id) ON DELETE SET NULL,
    FOREIGN KEY (parent_stage_item_id) REFERENCES production_stage_items(id) ON DELETE SET NULL,
    UNIQUE(stage_id, ordinal)
);

CREATE UNIQUE INDEX idx_production_stage_item_submission
    ON production_stage_items(stage_id, submission_idempotency_key)
    WHERE submission_idempotency_key IS NOT NULL;

CREATE INDEX idx_production_stage_items_task
    ON production_stage_items(task_id);

CREATE INDEX idx_production_stage_items_asset
    ON production_stage_items(asset_id);

CREATE INDEX idx_production_stage_items_source_asset
    ON production_stage_items(source_asset_id);

CREATE TABLE production_run_templates (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    krea2_workflow_version_id TEXT,
    krea2_recipe_id TEXT,
    krea2_preset_id TEXT,
    default_image_count INTEGER NOT NULL DEFAULT 1 CHECK (default_image_count BETWEEN 1 AND 100),
    h3_workflow_version_id TEXT,
    h3_recipe_id TEXT,
    h3_profile TEXT,
    default_duration_seconds INTEGER CHECK (default_duration_seconds BETWEEN 1 AND 15),
    default_width INTEGER,
    default_height INTEGER,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (krea2_workflow_version_id) REFERENCES workflow_versions(id) ON DELETE SET NULL,
    FOREIGN KEY (krea2_recipe_id) REFERENCES recipes(id) ON DELETE SET NULL,
    FOREIGN KEY (krea2_preset_id) REFERENCES presets(id) ON DELETE SET NULL,
    FOREIGN KEY (h3_workflow_version_id) REFERENCES workflow_versions(id) ON DELETE SET NULL,
    FOREIGN KEY (h3_recipe_id) REFERENCES recipes(id) ON DELETE SET NULL,
    UNIQUE(project_id, name)
);

CREATE INDEX idx_production_run_templates_project
    ON production_run_templates(project_id, updated_at DESC, id ASC);
