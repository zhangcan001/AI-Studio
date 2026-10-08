import { productRequest, productMediaUrl } from "./transport";
import type { CreationSubmission, CreationShotUpdate, GeneratorBindingSetRequest, RunRef, RunRetryRequest, RunListFilter } from "./types";

export const productClient = {
  library: {
    mediaVerify: (projectId: string, resource: import("./libraryTypes").ResourceRef) => productRequest("product_library_media_verify", { projectId, resource }),
    tagsList: (projectId: string) => productRequest("product_library_tags_list", { projectId }),
    imageGet: (projectId: string, resource: import("./libraryTypes").ResourceRef) => productRequest("product_library_image_get", { projectId, resource }),
    thumbnailGet: (projectId: string, resource: import("./libraryTypes").ResourceRef) => productRequest("product_library_thumbnail_get", { projectId, resource }),
    relationsGet: (projectId: string, resource: import("./libraryTypes").ResourceRef) => productRequest("product_library_relations_get", { projectId, resource }),
    versionsGet: (projectId: string, resource: import("./libraryTypes").ResourceRef) => productRequest("product_library_versions_get", { projectId, resource }),
    useInCreation: (projectId: string, resource: import("./libraryTypes").ResourceRef) => productRequest("product_library_use_in_creation", { projectId, resource }),
    deletionInspect: (projectId: string, resource: import("./libraryTypes").ResourceRef) => productRequest("product_library_deletion_inspect", { projectId, resource }),
    delete: (projectId: string, resource: import("./libraryTypes").ResourceRef, confirmed: boolean) => productRequest("product_library_delete", { projectId, resource, confirmed }),
    resourceEdit: (projectId: string, request: import("./libraryTypes").LibraryEditRequest) => productRequest("product_library_resource_edit", { projectId, request }),
    list: (projectId: string, query: import("./libraryTypes").LibraryQuery) => productRequest("product_library_list", { projectId, query }),
    get: (projectId: string, resource: import("./libraryTypes").ResourceRef) => productRequest("product_library_get", { projectId, resource }),
    mediaUrl: productMediaUrl,
  },
  project: {
    getOverview: (projectId: string) => productRequest("product_project_overview", { projectId }),
    generatorBindingSet: (projectId: string, request: GeneratorBindingSetRequest) => productRequest("product_generator_binding_set", { projectId, request }),
  },
  creation: {
    inputsGet: (selection: import("../types/shotVideoInput").VideoInputSelection) => productRequest("product_creation_inputs_get", { selection }),
    inputsSave: (request: import("../types/shotVideoInput").VideoInputSave) => productRequest("product_creation_inputs_save", { request }),
    assetsImport: (projectId: string, folder = false) => productRequest("product_creation_assets_import", { projectId, folder }),
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
