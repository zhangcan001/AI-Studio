-- Local Tool Hub metadata is additive and does not execute or install tools.
-- Existing ComfyUI settings/runtime services remain the authority for ComfyUI.
CREATE TABLE tools (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    type TEXT NOT NULL CHECK (length(trim(type)) > 0),
    description TEXT NOT NULL DEFAULT '',
    metadata_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_tools_name
    ON tools(name COLLATE NOCASE, id ASC);

CREATE TABLE tool_versions (
    id TEXT PRIMARY KEY NOT NULL,
    tool_id TEXT NOT NULL,
    version TEXT NOT NULL CHECK (length(trim(version)) > 0),
    observed_at TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    FOREIGN KEY (tool_id) REFERENCES tools(id) ON DELETE RESTRICT,
    UNIQUE (tool_id, version)
);

CREATE INDEX idx_tool_versions_tool_observed
    ON tool_versions(tool_id, observed_at DESC, id DESC);

CREATE TABLE tool_capabilities (
    tool_id TEXT NOT NULL,
    capability_name TEXT NOT NULL CHECK (length(trim(capability_name)) > 0),
    metadata_json TEXT NOT NULL,
    PRIMARY KEY (tool_id, capability_name),
    FOREIGN KEY (tool_id) REFERENCES tools(id) ON DELETE RESTRICT
);

CREATE INDEX idx_tool_capabilities_name
    ON tool_capabilities(capability_name COLLATE NOCASE, tool_id ASC);

CREATE TABLE tool_instances (
    id TEXT PRIMARY KEY NOT NULL,
    tool_id TEXT NOT NULL,
    path TEXT,
    endpoint TEXT,
    status TEXT NOT NULL DEFAULT 'UNKNOWN' CHECK (
        status IN ('AVAILABLE', 'MISSING', 'UNKNOWN')
    ),
    last_checked TEXT,
    CHECK (path IS NULL OR length(trim(path)) > 0),
    CHECK (endpoint IS NULL OR length(trim(endpoint)) > 0),
    FOREIGN KEY (tool_id) REFERENCES tools(id) ON DELETE RESTRICT
);

CREATE INDEX idx_tool_instances_tool_status
    ON tool_instances(tool_id, status, id ASC);

CREATE INDEX idx_tool_instances_last_checked
    ON tool_instances(last_checked DESC, id ASC);
