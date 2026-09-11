CREATE TABLE external_production_handoffs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version = 1),
    source_agent TEXT NOT NULL,
    source_revision TEXT,
    document_sha256 TEXT NOT NULL,
    imported_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    UNIQUE (project_id, document_sha256),
    UNIQUE (project_id, source_agent, source_revision)
);

CREATE INDEX idx_external_production_handoffs_project_imported
ON external_production_handoffs(project_id, imported_at DESC, id DESC);

CREATE TABLE external_production_handoff_entities (
    handoff_id TEXT NOT NULL,
    entity_kind TEXT NOT NULL CHECK (entity_kind IN ('series', 'episode', 'scene', 'shot')),
    external_id TEXT NOT NULL,
    formal_entity_id TEXT NOT NULL,
    FOREIGN KEY (handoff_id) REFERENCES external_production_handoffs(id) ON DELETE CASCADE,
    UNIQUE (handoff_id, entity_kind, external_id),
    UNIQUE (handoff_id, entity_kind, formal_entity_id)
);

CREATE INDEX idx_external_production_handoff_entities_handoff
ON external_production_handoff_entities(handoff_id, entity_kind, external_id);
