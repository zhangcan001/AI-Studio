-- Prompt Studio model metadata is a personal, user-maintained registry. It
-- records external model identity and capabilities; it does not execute or
-- install models.
CREATE TABLE models (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    provider TEXT NOT NULL CHECK (length(trim(provider)) > 0),
    type TEXT NOT NULL CHECK (length(trim(type)) > 0),
    description TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(provider, name)
);

CREATE INDEX idx_models_provider_name
    ON models(provider COLLATE NOCASE, name COLLATE NOCASE, id ASC);

CREATE TABLE model_versions (
    id TEXT PRIMARY KEY NOT NULL,
    model_id TEXT NOT NULL,
    version TEXT NOT NULL CHECK (length(trim(version)) > 0),
    capabilities_json TEXT NOT NULL,
    parameter_schema_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE RESTRICT,
    UNIQUE(model_id, version)
);

CREATE INDEX idx_model_versions_model_created
    ON model_versions(model_id, created_at DESC, id DESC);

ALTER TABLE prompt_versions
    ADD COLUMN model_version_id TEXT
    REFERENCES model_versions(id) ON DELETE SET NULL;

CREATE INDEX idx_prompt_versions_model_version
    ON prompt_versions(model_version_id);

ALTER TABLE generation_snapshots
    ADD COLUMN model_version_id TEXT
    REFERENCES model_versions(id) ON DELETE SET NULL;

CREATE INDEX idx_generation_snapshots_model_version
    ON generation_snapshots(model_version_id);
