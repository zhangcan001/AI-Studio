CREATE TABLE consistency_scope_profile_bindings (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('PROJECT', 'SERIES', 'EPISODE', 'SCENE')),
    scope_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('CHARACTER', 'SCENE', 'PROP', 'STYLE')),
    profile_type TEXT NOT NULL CHECK (profile_type IN ('CHARACTER', 'SCENE', 'PROP', 'STYLE')),
    profile_id TEXT NOT NULL,
    costume_variant_id TEXT,
    ordinal INTEGER NOT NULL DEFAULT 0 CHECK (ordinal >= 0),
    inheritance_mode TEXT NOT NULL DEFAULT 'EXPLICIT'
        CHECK (inheritance_mode IN ('EXPLICIT', 'INHERITED', 'REPLACE', 'REMOVE')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (costume_variant_id) REFERENCES costume_variants(id) ON DELETE RESTRICT,
    UNIQUE (project_id, scope_type, scope_id, role, ordinal, profile_id)
);

CREATE TABLE consistency_scope_reference_set_bindings (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    scope_type TEXT NOT NULL CHECK (scope_type IN ('PROJECT', 'SERIES', 'EPISODE', 'SCENE')),
    scope_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('CHARACTER', 'SCENE', 'PROP', 'STYLE', 'SHOT_REFERENCE')),
    reference_set_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL DEFAULT 0 CHECK (ordinal >= 0),
    required INTEGER NOT NULL DEFAULT 0 CHECK (required IN (0, 1)),
    inheritance_mode TEXT NOT NULL DEFAULT 'EXPLICIT'
        CHECK (inheritance_mode IN ('EXPLICIT', 'INHERITED', 'REPLACE', 'REMOVE')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (reference_set_id) REFERENCES reference_sets(id) ON DELETE RESTRICT,
    UNIQUE (project_id, scope_type, scope_id, role, ordinal, reference_set_id)
);

CREATE INDEX idx_consistency_scope_profile_bindings_scope
    ON consistency_scope_profile_bindings(project_id, scope_type, scope_id, role, ordinal);
CREATE INDEX idx_consistency_scope_profile_bindings_profile
    ON consistency_scope_profile_bindings(project_id, profile_type, profile_id);
CREATE INDEX idx_consistency_scope_reference_set_bindings_scope
    ON consistency_scope_reference_set_bindings(project_id, scope_type, scope_id, role, ordinal);
CREATE INDEX idx_consistency_scope_reference_set_bindings_reference_set
    ON consistency_scope_reference_set_bindings(project_id, reference_set_id);
