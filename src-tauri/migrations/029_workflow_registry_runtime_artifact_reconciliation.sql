-- Migration 028 copied workflow_versions.package_name to every recipe.  That
-- value is compatibility/history metadata, not runtime provenance.  Remove
-- only rows whose complete ID matches 028's provisional-ID expression; a
-- normal wra_<UUID> artifact must never be removed by this migration.
DELETE FROM workflow_runtime_artifacts
WHERE id = 'wra_' || workflow_version_id || '_' || recipe_id || '_' || package_name;

-- 028 did not persist a separate canonical-artifact column.  Its Registry
-- selection is the package recorded on the version row.  Use that value only
-- as a one-time migration bridge: it may select an already-existing real
-- artifact, but it never creates an artifact and is not a post-029 runtime
-- fallback.  A same-SHA duplicate without that explicit selection, or any
-- different-SHA duplicate, is unsafe to guess (by id, package name, or
-- insertion order) and must fail with a stable diagnostic.
CREATE TEMP TABLE dev084_runtime_artifact_pair_conflicts (
    workflow_version_id TEXT NOT NULL,
    recipe_id TEXT NOT NULL
);

CREATE TEMP TRIGGER dev084_runtime_artifact_pair_conflict_guard
BEFORE INSERT ON dev084_runtime_artifact_pair_conflicts
BEGIN
    SELECT RAISE(ABORT, 'RUNTIME_ARTIFACT_CONFLICT: multiple runtime packages claim one exact recipe');
END;

INSERT INTO dev084_runtime_artifact_pair_conflicts (workflow_version_id, recipe_id)
SELECT workflow_version_id, recipe_id
FROM workflow_runtime_artifacts
GROUP BY workflow_version_id, recipe_id
HAVING COUNT(*) > 1
   AND COUNT(DISTINCT workflow_sha256 || char(0) || recipe_sha256) > 1;

INSERT INTO dev084_runtime_artifact_pair_conflicts (workflow_version_id, recipe_id)
SELECT artifacts.workflow_version_id, artifacts.recipe_id
FROM workflow_runtime_artifacts AS artifacts
INNER JOIN workflow_versions AS versions
    ON versions.id = artifacts.workflow_version_id
GROUP BY artifacts.workflow_version_id, artifacts.recipe_id
HAVING COUNT(*) > 1
   AND COUNT(DISTINCT artifacts.workflow_sha256 || char(0) || artifacts.recipe_sha256) = 1
   AND SUM(
       CASE WHEN artifacts.package_name = versions.package_name THEN 1 ELSE 0 END
   ) <> 1;

-- Same-SHA duplicates are safe to compact only when the historical Registry
-- selection points to exactly one of the already-existing artifact rows.  The
-- predicate names the canonical row directly; it does not use ORDER BY/LIMIT
-- or any arbitrary row-number rule.
DELETE FROM workflow_runtime_artifacts AS duplicate
WHERE EXISTS (
    SELECT 1
    FROM workflow_runtime_artifacts AS canonical
    INNER JOIN workflow_versions AS versions
        ON versions.id = canonical.workflow_version_id
    WHERE canonical.workflow_version_id = duplicate.workflow_version_id
      AND canonical.recipe_id = duplicate.recipe_id
      AND canonical.package_name = versions.package_name
      AND canonical.workflow_sha256 = duplicate.workflow_sha256
      AND canonical.recipe_sha256 = duplicate.recipe_sha256
      AND duplicate.package_name <> canonical.package_name
);

DROP TRIGGER dev084_runtime_artifact_pair_conflict_guard;
DROP TABLE dev084_runtime_artifact_pair_conflicts;

-- 028 created a non-unique index with this name.  Replace it with the
-- cardinality invariant: one exact (workflow_version_id, recipe_id) has one
-- runtime artifact.  UNIQUE(package_name), defined on the table in 028, is
-- intentionally retained as the package identity invariant.
DROP INDEX idx_workflow_runtime_artifacts_version_recipe;
CREATE UNIQUE INDEX idx_workflow_runtime_artifacts_version_recipe
    ON workflow_runtime_artifacts(workflow_version_id, recipe_id);
