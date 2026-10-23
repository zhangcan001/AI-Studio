-- Writer-provided H3 Ref2VA sections per shot. NULL = use compiler fallback.
-- ADD COLUMN with NULL default: no table rebuild, no existing row rewritten.
ALTER TABLE shots ADD COLUMN prompt_style TEXT;
ALTER TABLE shots ADD COLUMN prompt_summary TEXT;
ALTER TABLE shots ADD COLUMN prompt_soundscape TEXT;
ALTER TABLE shots ADD COLUMN prompt_music TEXT;
