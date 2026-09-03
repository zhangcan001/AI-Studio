import type { RecipeViewModel } from "../../types/generation";
import {
  findRecipe,
  recipeRef,
  recipesForVideoMode,
  sameRecipeRef,
  videoRecipeCapability,
  type H3CompatibleMode,
  type SelectedRecipeRef,
} from "./workflowCapabilities";

export type ProjectWorkflowResolutionSource =
  | "explicit"
  | "project_mode"
  | "project_default"
  | "recommended"
  | "compatible";

export interface ProjectWorkflowResolution<T extends RecipeViewModel = RecipeViewModel> {
  recipe?: T;
  source?: ProjectWorkflowResolutionSource;
  staleProjectBinding: boolean;
}

interface ResolutionOptions<T extends RecipeViewModel> {
  candidates: T[];
  explicit?: SelectedRecipeRef;
  projectMode?: SelectedRecipeRef;
  projectDefault?: SelectedRecipeRef;
  recommended?: T;
}

export interface ProjectVideoWorkflowResolutionOptions {
  allowGenericFallback?: boolean;
}

export function resolveProjectWorkflow<T extends RecipeViewModel>({
  candidates,
  explicit,
  projectMode,
  projectDefault,
  recommended,
}: ResolutionOptions<T>): ProjectWorkflowResolution<T> {
  const explicitRecipe = findRecipe(candidates, explicit) as T | undefined;
  if (explicitRecipe) {
    return { recipe: explicitRecipe, source: "explicit", staleProjectBinding: false };
  }

  const projectModeRecipe = findRecipe(candidates, projectMode) as T | undefined;
  if (projectModeRecipe) {
    return {
      recipe: projectModeRecipe,
      source: "project_mode",
      staleProjectBinding: false,
    };
  }

  const projectDefaultRecipe = findRecipe(candidates, projectDefault) as T | undefined;
  if (projectDefaultRecipe) {
    return {
      recipe: projectDefaultRecipe,
      source: "project_default",
      staleProjectBinding: Boolean(projectMode && !projectModeRecipe),
    };
  }

  const recommendedRecipe = recommended && candidates.some((candidate) => sameRecipeRef(candidate, recipeRef(recommended)))
    ? recommended
    : undefined;
  if (recommendedRecipe) {
    return {
      recipe: recommendedRecipe,
      source: "recommended",
      staleProjectBinding: Boolean(
        (projectMode && !projectModeRecipe) || (projectDefault && !projectDefaultRecipe),
      ),
    };
  }

  return {
    recipe: candidates[0],
    source: candidates[0] ? "compatible" : undefined,
    staleProjectBinding: Boolean(
      (projectMode && !projectModeRecipe) || (projectDefault && !projectDefaultRecipe),
    ),
  };
}

export function resolveProjectImageWorkflow<T extends RecipeViewModel>(
  catalog: T[],
  explicit?: SelectedRecipeRef,
  projectDefault?: SelectedRecipeRef,
  recommended?: T,
): ProjectWorkflowResolution<T> {
  return resolveProjectWorkflow({
    candidates: catalog,
    explicit,
    projectDefault,
    recommended,
  });
}

export function resolveProjectVideoWorkflow<T extends RecipeViewModel>(
  catalog: T[],
  mode: H3CompatibleMode | "CUSTOM_VIDEO",
  explicit?: SelectedRecipeRef,
  projectMode?: SelectedRecipeRef,
  projectDefault?: SelectedRecipeRef,
  recommended?: T,
  options: ProjectVideoWorkflowResolutionOptions = {},
): ProjectWorkflowResolution<T> {
  const modeCandidates = recipesForVideoMode(catalog, mode) as T[];
  const genericVideoCandidates = recipesForVideoMode(catalog, "CUSTOM_VIDEO") as T[];
  const candidates = modeCandidates.length || mode === "CUSTOM_VIDEO"
    ? modeCandidates
    : options.allowGenericFallback === false
      ? []
      : genericVideoCandidates;
  const resolution = resolveProjectWorkflow({
    candidates,
    explicit,
    projectMode,
    projectDefault,
    recommended,
  });
  const staleProjectMode = Boolean(projectMode && !findRecipe(candidates, projectMode));
  const staleProjectDefault = Boolean(
    projectDefault
      && !findRecipe(candidates, projectDefault)
      && !findRecipe(genericVideoCandidates, projectDefault),
  );
  return {
    ...resolution,
    staleProjectBinding: staleProjectMode || staleProjectDefault,
  };
}

export function resolveProjectFolderWorkflow<T extends RecipeViewModel>(
  catalog: T[],
  mode: H3CompatibleMode,
  explicit?: SelectedRecipeRef,
  projectMode?: SelectedRecipeRef,
  projectDefault?: SelectedRecipeRef,
  recommended?: T,
): ProjectWorkflowResolution<T> {
  const candidates = recipesForVideoMode(catalog, mode).filter((recipe) => (
    videoRecipeCapability(recipe).projectFolderModes.includes(mode)
  )) as T[];
  return resolveProjectWorkflow({
    candidates,
    explicit,
    projectMode,
    projectDefault,
    recommended,
  });
}
