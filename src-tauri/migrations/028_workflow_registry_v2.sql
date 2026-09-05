-- Workflow Registry V2: logical workflow metadata and exact runtime-package
-- provenance. The package columns on workflow_versions remain compatibility
-- fields only; they are not an exact artifact lookup source.
ALTER TABLE workflows ADD COLUMN source_kind TEXT NOT NULL DEFAULT 'USER'
    CHECK (source_kind IN ('PRODUCT', 'USER'));

ALTER TABLE workflows ADD COLUMN library_state TEXT NOT NULL DEFAULT 'ACTIVE'
    CHECK (library_state IN ('ACTIVE', 'REMOVED'));

ALTER TABLE workflows ADD COLUMN removed_at TEXT;

-- Existing databases do not have first-class source metadata. This is a
-- one-time legacy backfill for the product package identities known to the
-- embedded runtime; future ingestion must persist source_kind directly.
UPDATE workflows
SET source_kind = CASE
    WHEN EXISTS (
        SELECT 1
        FROM workflow_versions wv
        WHERE wv.workflow_id = workflows.id
          AND wv.package_name IN (
              'minimax_h3_fl2va_1_0_0',
              'minimax_h3_reference_video_1_3_0',
              'minimax_h3_fl2va_t2v_quality_2_0_0',
              'minimax_h3_fl2va_i2v_quality_2_0_0',
              'minimax_h3_fl2va_first_last_quality_2_0_0',
              'minimax_h3_reference_video_quality_2_0_0',
              'aitudou_minimax_h3_lightx2v_8step_fast_1_0_0',
              'kera2_t2i_local_v2',
              'kera2_t2i_local_v2_1_1_0_1d99a10d',
              'krea2_t2i_local'
          )
    ) THEN 'PRODUCT'
    ELSE 'USER'
END;

-- A logical workflow is removed only when every known version is archived.
-- Missing legacy runtime-state rows are treated as active for safety.
UPDATE workflows
SET library_state = CASE
    WHEN EXISTS (
        SELECT 1
        FROM workflow_versions wv
        WHERE wv.workflow_id = workflows.id
    )
    AND NOT EXISTS (
        SELECT 1
        FROM workflow_versions wv
        LEFT JOIN workflow_runtime_states wrs
            ON wrs.workflow_version_id = wv.id
        WHERE wv.workflow_id = workflows.id
          AND COALESCE(wrs.archived, 0) = 0
    ) THEN 'REMOVED'
    ELSE 'ACTIVE'
END;

UPDATE workflows
SET removed_at = CASE
    WHEN library_state = 'REMOVED' THEN COALESCE(removed_at, updated_at)
    ELSE NULL
END;

CREATE TABLE workflow_runtime_artifacts (
    id TEXT PRIMARY KEY,
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL,
    package_name TEXT NOT NULL,
    source_kind TEXT NOT NULL
        CHECK (source_kind IN ('PRODUCT', 'USER')),
    package_source_path TEXT,
    workflow_sha256 TEXT NOT NULL,
    recipe_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL,

    FOREIGN KEY (workflow_version_id)
        REFERENCES workflow_versions(id),
    FOREIGN KEY (recipe_id)
        REFERENCES recipes(id),
    UNIQUE (package_name),
    UNIQUE (workflow_version_id, recipe_id, package_name)
);

CREATE INDEX idx_workflow_runtime_artifacts_version_recipe
    ON workflow_runtime_artifacts(workflow_version_id, recipe_id);

CREATE INDEX idx_workflow_runtime_artifacts_package
    ON workflow_runtime_artifacts(package_name);

-- Preserve every legacy workflowVersionId and recipeId. A legacy version had
-- at most one package column, so that value is copied to each recipe as the
-- safest exact tuple available; later syncs can add additional packages
-- without overwriting these rows.
INSERT OR IGNORE INTO workflow_runtime_artifacts (
    id,
    workflow_version_id,
    recipe_id,
    package_name,
    source_kind,
    package_source_path,
    workflow_sha256,
    recipe_sha256,
    created_at
)
SELECT
    'wra_' || wv.id || '_' || r.id || '_' || wv.package_name,
    wv.id,
    r.id,
    wv.package_name,
    w.source_kind,
    wv.package_source_path,
    wv.workflow_sha256,
    r.recipe_sha256,
    COALESCE(r.created_at, wv.created_at)
FROM workflow_versions wv
INNER JOIN workflows w ON w.id = wv.workflow_id
INNER JOIN recipes r ON r.workflow_version_id = wv.id
WHERE wv.package_name IS NOT NULL
  AND length(trim(wv.package_name)) > 0;
