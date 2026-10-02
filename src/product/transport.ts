import { invokeCommand } from "../services/ipc";
import { normalizeProductError } from "./errors";
import type { CreationSubmission, CreationReadiness, CreationAccepted, CreationContext, CreationShot, CreationShotSummary, CreationShotUpdate, GeneratorBindingSetRequest, GeneratorBindingSummary, GeneratorOption, ProductRun, ProjectOverview, RunRef, RunRetryRequest } from "./types";

interface ProductCommands {
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
