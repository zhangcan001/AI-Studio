-- Optional recipe promotion is independent from workflow_versions.current_version_id.
-- A missing row means that no recipe is promoted for the workflow version.
CREATE UNIQUE INDEX idx_recipes_workflow_version_id_id
    ON recipes(workflow_version_id, id);

CREATE TABLE workflow_recipe_promotions (
    workflow_version_id TEXT PRIMARY KEY,
    recipe_id TEXT NOT NULL,
    promoted_at TEXT NOT NULL,

    FOREIGN KEY (workflow_version_id)
        REFERENCES workflow_versions(id)
        ON DELETE CASCADE,
    FOREIGN KEY (workflow_version_id, recipe_id)
        REFERENCES recipes(workflow_version_id, id)
        ON DELETE CASCADE
);

CREATE INDEX idx_workflow_recipe_promotions_recipe
    ON workflow_recipe_promotions(workflow_version_id, recipe_id);
