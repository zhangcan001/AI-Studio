-- 构图用途: optional per-shot usage of a bound composition entity.
-- 'opening' (开场构图) governs only the starting framing; 'ending' (结尾构图)
-- governs only the final frame. Existing bindings keep NULL, so prompt
-- compilation is unchanged for them. ADD COLUMN with a NULL default and a CHECK
-- needs no table rebuild, so no binding row is rewritten or lost.
ALTER TABLE shot_story_entity_bindings
    ADD COLUMN composition_usage TEXT
    CHECK (composition_usage IS NULL OR composition_usage IN ('opening', 'ending'));
-- At most one opening and one ending per shot; UNIQUE(shot_id, entity_id)
-- already prevents one entity from holding both usages.
CREATE UNIQUE INDEX idx_shot_story_entity_bindings_composition_usage
    ON shot_story_entity_bindings(shot_id, composition_usage)
    WHERE composition_usage IS NOT NULL;
