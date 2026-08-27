-- Script/Draft data foundation. Draft revisions are immutable documents; the
-- formal production structure remains deliberately outside this migration.
CREATE TABLE script_sources (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    format TEXT NOT NULL CHECK (format IN ('TXT', 'MARKDOWN', 'JSON')),
    original_filename TEXT,
    source_checksum TEXT NOT NULL
        CHECK (
            length(source_checksum) = 64
            AND source_checksum NOT GLOB '*[^0-9a-f]*'
        ),
    source_bytes INTEGER NOT NULL CHECK (source_bytes >= 0),
    source_text TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    created_at TEXT NOT NULL,

    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    UNIQUE (id, project_id),
    UNIQUE (project_id, source_checksum, format)
);

CREATE INDEX idx_script_sources_project_created
    ON script_sources(project_id, created_at DESC, id ASC);

CREATE TABLE script_import_drafts (
    id TEXT PRIMARY KEY NOT NULL,
    draft_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision > 0),
    previous_revision_id TEXT,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    revision_kind TEXT NOT NULL CHECK (
        revision_kind IN ('PARSED', 'REPARSED', 'USER_EDIT', 'REVIEW', 'MERGE', 'SPLIT', 'REORDER')
    ),
    parser_version TEXT NOT NULL,
    contract_version INTEGER NOT NULL CHECK (contract_version > 0),
    provider_kind TEXT,
    provider_model TEXT,
    provider_metadata_json TEXT,
    payload_checksum TEXT NOT NULL
        CHECK (
            length(payload_checksum) = 64
            AND payload_checksum NOT GLOB '*[^0-9a-f]*'
        ),
    summary_json TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,

    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (source_id, project_id)
        REFERENCES script_sources(id, project_id) ON DELETE CASCADE,
    FOREIGN KEY (project_id, draft_id, previous_revision_id)
        REFERENCES script_import_drafts(project_id, draft_id, id),
    UNIQUE (project_id, draft_id, id),
    UNIQUE (project_id, draft_id, revision)
);

CREATE INDEX idx_script_import_drafts_project_created
    ON script_import_drafts(project_id, created_at DESC, draft_id ASC, revision DESC);

CREATE INDEX idx_script_import_drafts_draft_revision
    ON script_import_drafts(project_id, draft_id, revision DESC);

-- Revisions are append-only. Project/source cascades may remove rows, but no
-- caller can mutate a revision (including its payload) in place.
CREATE TRIGGER script_import_drafts_immutable_update
BEFORE UPDATE ON script_import_drafts
BEGIN
    SELECT RAISE(ABORT, 'script_import_drafts are immutable');
END;
