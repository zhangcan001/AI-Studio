ALTER TABLE workflow_versions ADD COLUMN package_source_path TEXT;

ALTER TABLE tasks ADD COLUMN app_version TEXT;
ALTER TABLE tasks ADD COLUMN build_commit TEXT;
ALTER TABLE tasks ADD COLUMN workflow_version TEXT;
ALTER TABLE tasks ADD COLUMN workflow_sha256 TEXT;
ALTER TABLE tasks ADD COLUMN recipe_version TEXT;
ALTER TABLE tasks ADD COLUMN recipe_sha256 TEXT;
ALTER TABLE tasks ADD COLUMN package_name TEXT;
ALTER TABLE tasks ADD COLUMN package_source_path TEXT;
ALTER TABLE tasks ADD COLUMN dynamic_binding_targets_json TEXT;
