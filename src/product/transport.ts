import { invokeCommand } from "../services/ipc";
import { normalizeProductError } from "./errors";
import type { GeneratorBindingSetRequest, GeneratorBindingSummary, GeneratorOption, ProductRun, ProjectOverview, RunRef, RunRetryRequest } from "./types";

interface ProductCommands {
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
