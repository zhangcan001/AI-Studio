CREATE TABLE profile_revisions (
    id TEXT PRIMARY KEY,
    profile_type TEXT NOT NULL CHECK (profile_type IN ('CHARACTER', 'SCENE', 'PROP', 'STYLE')),
    profile_id TEXT NOT NULL,
    revision_number INTEGER NOT NULL CHECK (revision_number >= 1),
    content_json TEXT NOT NULL,
    content_sha256 TEXT NOT NULL CHECK (length(trim(content_sha256)) > 0),
    status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'ARCHIVED')),
    created_at TEXT NOT NULL,
    created_by TEXT,
    UNIQUE (profile_type, profile_id, revision_number)
);

CREATE TABLE reference_sets (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    purpose TEXT NOT NULL CHECK (purpose IN ('CHARACTER', 'COSTUME', 'SCENE', 'PROP', 'STYLE', 'SHOT')),
    description TEXT NOT NULL DEFAULT '',
    owner_profile_type TEXT CHECK (owner_profile_type IN ('CHARACTER', 'SCENE', 'PROP', 'STYLE')),
    owner_profile_id TEXT,
    active_revision_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE style_profiles (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    style_prompt TEXT NOT NULL,
    color_prompt TEXT,
    line_prompt TEXT,
    negative_prompt TEXT,
    output_notes TEXT,
    active_revision_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

CREATE TABLE character_profiles (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    canonical_prompt TEXT NOT NULL,
    negative_prompt TEXT NOT NULL,
    default_style_profile_id TEXT,
    default_reference_set_id TEXT,
    active_revision_id TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (default_style_profile_id) REFERENCES style_profiles(id) ON DELETE RESTRICT,
    FOREIGN KEY (default_reference_set_id) REFERENCES reference_sets(id) ON DELETE RESTRICT
);

CREATE TABLE scene_profiles (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    environment_prompt TEXT NOT NULL,
    lighting_prompt TEXT,
    negative_prompt TEXT,
    default_style_profile_id TEXT,
    default_reference_set_id TEXT,
    active_revision_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (default_style_profile_id) REFERENCES style_profiles(id) ON DELETE RESTRICT,
    FOREIGN KEY (default_reference_set_id) REFERENCES reference_sets(id) ON DELETE RESTRICT
);

CREATE TABLE prop_profiles (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    canonical_prompt TEXT NOT NULL,
    material_prompt TEXT,
    scale_prompt TEXT,
    default_reference_set_id TEXT,
    active_revision_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE,
    FOREIGN KEY (default_reference_set_id) REFERENCES reference_sets(id) ON DELETE RESTRICT
);

CREATE TABLE costume_variants (
    id TEXT PRIMARY KEY,
    character_profile_id TEXT NOT NULL,
    name TEXT NOT NULL,
    prompt_fragment TEXT NOT NULL,
    reference_set_id TEXT,
    is_default INTEGER NOT NULL DEFAULT 0 CHECK (is_default IN (0, 1)),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    active_revision_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (character_profile_id) REFERENCES character_profiles(id) ON DELETE CASCADE,
    FOREIGN KEY (reference_set_id) REFERENCES reference_sets(id) ON DELETE RESTRICT
);

CREATE TABLE reference_set_items (
    reference_set_id TEXT NOT NULL,
    asset_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    role TEXT,
    is_primary INTEGER NOT NULL DEFAULT 0 CHECK (is_primary IN (0, 1)),
    created_at TEXT NOT NULL,
    PRIMARY KEY (reference_set_id, asset_id),
    UNIQUE (reference_set_id, ordinal),
    FOREIGN KEY (reference_set_id) REFERENCES reference_sets(id) ON DELETE CASCADE,
    FOREIGN KEY (asset_id) REFERENCES assets(id) ON DELETE RESTRICT
);

CREATE TABLE shot_profile_bindings (
    id TEXT PRIMARY KEY,
    shot_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('CHARACTER', 'SCENE', 'PROP', 'STYLE')),
    profile_type TEXT NOT NULL CHECK (profile_type IN ('CHARACTER', 'SCENE', 'PROP', 'STYLE')),
    profile_id TEXT NOT NULL,
    costume_variant_id TEXT,
    ordinal INTEGER NOT NULL DEFAULT 0 CHECK (ordinal >= 0),
    inheritance_mode TEXT NOT NULL DEFAULT 'EXPLICIT'
        CHECK (inheritance_mode IN ('EXPLICIT', 'INHERITED', 'REPLACE', 'REMOVE')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE,
    FOREIGN KEY (costume_variant_id) REFERENCES costume_variants(id) ON DELETE RESTRICT,
    UNIQUE (shot_id, role, ordinal, profile_id)
);

CREATE TABLE shot_reference_set_bindings (
    id TEXT PRIMARY KEY,
    shot_id TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('CHARACTER', 'SCENE', 'PROP', 'STYLE', 'SHOT_REFERENCE')),
    reference_set_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL DEFAULT 0 CHECK (ordinal >= 0),
    required INTEGER NOT NULL DEFAULT 0 CHECK (required IN (0, 1)),
    inheritance_mode TEXT NOT NULL DEFAULT 'EXPLICIT'
        CHECK (inheritance_mode IN ('EXPLICIT', 'INHERITED', 'REPLACE', 'REMOVE')),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    FOREIGN KEY (shot_id) REFERENCES shots(id) ON DELETE CASCADE,
    FOREIGN KEY (reference_set_id) REFERENCES reference_sets(id) ON DELETE RESTRICT,
    UNIQUE (shot_id, role, ordinal, reference_set_id)
);

CREATE UNIQUE INDEX uq_character_profiles_project_name
    ON character_profiles(project_id, name COLLATE NOCASE);
CREATE UNIQUE INDEX uq_scene_profiles_project_name
    ON scene_profiles(project_id, name COLLATE NOCASE);
CREATE UNIQUE INDEX uq_prop_profiles_project_name
    ON prop_profiles(project_id, name COLLATE NOCASE);
CREATE UNIQUE INDEX uq_style_profiles_project_name
    ON style_profiles(project_id, name COLLATE NOCASE);
CREATE UNIQUE INDEX uq_reference_sets_project_name
    ON reference_sets(project_id, name COLLATE NOCASE);

CREATE INDEX idx_character_profiles_project_updated_id
    ON character_profiles(project_id, updated_at, id);
CREATE INDEX idx_scene_profiles_project_updated_id
    ON scene_profiles(project_id, updated_at, id);
CREATE INDEX idx_prop_profiles_project_updated_id
    ON prop_profiles(project_id, updated_at, id);
CREATE INDEX idx_style_profiles_project_updated_id
    ON style_profiles(project_id, updated_at, id);
CREATE INDEX idx_reference_sets_project_purpose_updated_id
    ON reference_sets(project_id, purpose, updated_at, id);
CREATE INDEX idx_reference_set_items_set_ordinal
    ON reference_set_items(reference_set_id, ordinal);
CREATE INDEX idx_profile_revisions_type_profile_revision
    ON profile_revisions(profile_type, profile_id, revision_number);
CREATE INDEX idx_shot_profile_bindings_shot_role_ordinal
    ON shot_profile_bindings(shot_id, role, ordinal);
CREATE INDEX idx_shot_reference_set_bindings_shot_role_ordinal
    ON shot_reference_set_bindings(shot_id, role, ordinal);
