import { productRequest } from "./transport";
import type { CreationShotUpdate, GeneratorBindingSetRequest, RunRef, RunRetryRequest } from "./types";

export const productClient = {
  project: {
    getOverview: (projectId: string) => productRequest("product_project_overview", { projectId }),
    generatorBindingSet: (projectId: string, request: GeneratorBindingSetRequest) => productRequest("product_generator_binding_set", { projectId, request }),
  },
  creation: {
    get: (projectId: string, shotId: string | null, stage: "image" | "video") => productRequest("product_creation_get", { projectId, shotId, stage }),
    createShot: (projectId: string) => productRequest("product_creation_shot_create", { projectId }),
    updateShot: (projectId: string, request: CreationShotUpdate) => productRequest("product_creation_shot_update", { projectId, request }),
    deleteShot: (projectId: string, shotId: string) => productRequest("product_creation_shot_delete", { projectId, shotId }),
    referencesSet: (projectId: string, shotId: string, stage: "image" | "video", assetIds: string[]) => productRequest("product_creation_references_set", { projectId, shotId, stage, assetIds }),
    selectResult: (projectId: string, shotId: string, stage: "image" | "video", assetId: string) => productRequest("product_creation_result_select", { projectId, shotId, stage, assetId }),
    generatorsList: (projectId: string, stage: "image" | "video", shotId: string | null = null) => productRequest("product_generators_list", { projectId, shotId, stage }),
  },
  run: {
    get: (projectId: string, runRef: RunRef) => productRequest("product_run_get", { projectId, runRef }),
    retry: (projectId: string, request: RunRetryRequest) => productRequest("product_run_retry", { projectId, request }),
  },
};
