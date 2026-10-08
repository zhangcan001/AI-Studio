import { invokeCommand } from "../services/ipc";
import { normalizeProductError } from "./errors";
import { buildAssetMediaUrl } from "../services/mediaUrl";

export const productMediaUrl = buildAssetMediaUrl;
import type { CreationSubmission, CreationReadiness, CreationAccepted, CreationContext, CreationShot, CreationShotSummary, CreationShotUpdate, GeneratorBindingSetRequest, GeneratorBindingSummary, GeneratorOption, ProductRun, ProjectOverview, RunRef, RunRetryRequest, RunList, RunListFilter } from "./types";

interface ProductCommands {
  product_creation_inputs_get: { args: { selection: import("../types/shotVideoInput").VideoInputSelection }; result: import("../types/shotVideoInput").VideoInputView };
  product_creation_inputs_save: { args: { request: import("../types/shotVideoInput").VideoInputSave }; result: import("../types/shotVideoInput").VideoInputView };
  product_creation_assets_import: { args: { projectId: string; folder: boolean }; result: import("../types/shotVideoInput").SourceInputImport };
  product_library_tags_list: { args: { projectId: string }; result: import("./libraryTypes").LibraryTag[] };
  product_library_image_get: { args: { projectId: string; resource: import("./libraryTypes").ResourceRef }; result: number[] };
  product_library_media_verify: { args: { projectId: string; resource: import("./libraryTypes").ResourceRef }; result: import("./libraryTypes").MediaIntegrityReport };
  product_library_thumbnail_get: { args: { projectId: string; resource: import("./libraryTypes").ResourceRef }; result: number[] };
  product_library_relations_get: { args: { projectId: string; resource: import("./libraryTypes").ResourceRef }; result: import("./libraryTypes").LibraryRelation[] };
  product_library_versions_get: { args: { projectId: string; resource: import("./libraryTypes").ResourceRef }; result: import("./libraryTypes").LibraryVersions };
  product_library_use_in_creation: { args: { projectId: string; resource: import("./libraryTypes").ResourceRef }; result: import("./libraryTypes").LibraryCreateIntent };
  product_library_deletion_inspect: { args: { projectId: string; resource: import("./libraryTypes").ResourceRef }; result: import("./libraryTypes").LibraryDeletionInspection };
  product_library_delete: { args: { projectId: string; resource: import("./libraryTypes").ResourceRef; confirmed: boolean }; result: void };
  product_library_resource_edit: { args: { projectId: string; request: import("./libraryTypes").LibraryEditRequest }; result: import("./libraryTypes").LibraryDetail };
  product_library_list: { args: { projectId: string; query: import("./libraryTypes").LibraryQuery }; result: import("./libraryTypes").LibraryList };
  product_library_get: { args: { projectId: string; resource: import("./libraryTypes").ResourceRef }; result: import("./libraryTypes").LibraryDetail };
  product_run_results_get: { args: { projectId: string; runRef: RunRef }; result: import("./types").RunResult[] };
  product_run_result_review: { args: { projectId: string; request: import("./types").RunResultReviewRequest }; result: void };
  product_run_list: { args: { projectId: string; filter: RunListFilter; cursor: string | null }; result: RunList };
  product_run_start: { args: { projectId: string; runRef: RunRef }; result: ProductRun };
  product_run_pause: { args: { projectId: string; runRef: RunRef }; result: ProductRun };
  product_run_cancel: { args: { projectId: string; runRef: RunRef }; result: ProductRun };
  product_creation_readiness_get: { args: { request: CreationSubmission }; result: CreationReadiness };
  product_creation_generate: { args: { request: CreationSubmission }; result: CreationAccepted };
  product_creation_get: { args: { projectId: string; shotId: string | null; stage: "image" | "video" }; result: CreationContext };
  product_creation_shot_create: { args: { projectId: string }; result: CreationShotSummary };
  product_creation_shot_update: { args: { projectId: string; request: CreationShotUpdate }; result: CreationShotSummary };
  product_creation_shot_delete: { args: { projectId: string; shotId: string }; result: void };
  product_creation_references_set: { args: { projectId: string; shotId: string; stage: "image" | "video"; assetIds: string[] }; result: CreationShot };
  product_creation_result_select: { args: { projectId: string; shotId: string; stage: "image" | "video"; assetId: string }; result: CreationShot };
  product_project_overview: { args: { projectId: string }; result: ProjectOverview };
  product_generators_list: { args: { projectId: string; shotId: string | null; stage: "image" | "video" }; result: GeneratorOption[] };
  product_generator_binding_set: { args: { projectId: string; request: GeneratorBindingSetRequest }; result: GeneratorBindingSummary[] };
  product_run_get: { args: { projectId: string; runRef: RunRef }; result: ProductRun };
  product_run_retry: { args: { projectId: string; request: RunRetryRequest }; result: ProductRun };
}
export async function productRequest<K extends keyof ProductCommands>(command: K, args: ProductCommands[K]["args"]): Promise<ProductCommands[K]["result"]> {
  try { return await invokeCommand<ProductCommands[K]["result"]>(command, args); }
  catch (error) { throw normalizeProductError(error); }
}
