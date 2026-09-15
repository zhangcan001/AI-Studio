-- Prompt provenance is stored only when a caller supplies the exact
-- PromptVersion identity. Historical snapshots remain unchanged and are not
-- backfilled from prompt text, filenames, or task metadata.
ALTER TABLE generation_snapshots
    ADD COLUMN prompt_version_id TEXT;

CREATE INDEX idx_generation_snapshots_prompt_version
    ON generation_snapshots(prompt_version_id);
