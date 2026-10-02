import { productRequest, productMediaUrl } from "./transport";
import type { CreationSubmission, CreationShotUpdate, GeneratorBindingSetRequest, RunRef, RunRetryRequest, RunListFilter } from "./types";

export const productClient = {
  project: {
    getOverview: (projectId: string) => productRequest("product_project_overview", { projectId }),
    generatorBindingSet: (projectId: string, request: GeneratorBindingSetRequest) => productRequest("product_generator_binding_set", { projectId, request }),
  },
  creation: {
    mediaUrl: productMediaUrl,
    readinessGet: (request: CreationSubmission) => productRequest("product_creation_readiness_get", { request }),
    generate: (request: CreationSubmission) => productRequest("product_creation_generate", { request }),
    get: (projectId: string, shotId: string | null, stage: "image" | "video") => productRequest("product_creation_get", { projectId, shotId, stage }),
    createShot: (projectId: string) => productRequest("product_creation_shot_create", { projectId }),
    updateShot: (projectId: string, request: CreationShotUpdate) => productRequest("product_creation_shot_update", { projectId, request }),
    deleteShot: (projectId: string, shotId: string) => productRequest("product_creation_shot_delete", { projectId, shotId }),
    referencesSet: (projectId: string, shotId: string, stage: "image" | "video", assetIds: string[]) => productRequest("product_creation_references_set", { projectId, shotId, stage, assetIds }),
    selectResult: (projectId: string, shotId: string, stage: "image" | "video", assetId: string) => productRequest("product_creation_result_select", { projectId, shotId, stage, assetId }),
    generatorsList: (projectId: string, stage: "image" | "video", shotId: string | null = null) => productRequest("product_generators_list", { projectId, shotId, stage }),
  },
  run: {
    resultsGet: (projectId: string, runRef: RunRef) => productRequest("product_run_results_get", { projectId, runRef }),
    resultReview: (projectId: string, request: import("./types").RunResultReviewRequest) => productRequest("product_run_result_review", { projectId, request }),
    list: (projectId: string, filter: RunListFilter = "all", cursor: string | null = null) => productRequest("product_run_list", { projectId, filter, cursor }),
    start: (projectId: string, runRef: RunRef) => productRequest("product_run_start", { projectId, runRef }),
    pause: (projectId: string, runRef: RunRef) => productRequest("product_run_pause", { projectId, runRef }),
    cancel: (projectId: string, runRef: RunRef) => productRequest("product_run_cancel", { projectId, runRef }),
    get: (projectId: string, runRef: RunRef) => productRequest("product_run_get", { projectId, runRef }),
    retry: (projectId: string, request: RunRetryRequest) => productRequest("product_run_retry", { projectId, request }),
  },
};
