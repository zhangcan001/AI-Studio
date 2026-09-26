-- Recognition provenance and the original imported payload belong to the
-- immutable WorkflowVersion. Existing rows remain readable with null metadata.
ALTER TABLE workflow_versions ADD COLUMN source_workflow_json TEXT;
ALTER TABLE workflow_versions ADD COLUMN recognition_metadata_json TEXT;
