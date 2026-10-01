import { productRequest } from "./transport";
import type { GeneratorBindingSetRequest, RunRef, RunRetryRequest } from "./types";

export const productClient = {
  project: {
    getOverview: (projectId: string) => productRequest("product_project_overview", { projectId }),
    generatorBindingSet: (projectId: string, request: GeneratorBindingSetRequest) => productRequest("product_generator_binding_set", { projectId, request }),
  },
  creation: {
    generatorsList: (projectId: string, stage: "image" | "video", shotId: string | null = null) => productRequest("product_generators_list", { projectId, shotId, stage }),
  },
  run: {
    get: (projectId: string, runRef: RunRef) => productRequest("product_run_get", { projectId, runRef }),
    retry: (projectId: string, request: RunRetryRequest) => productRequest("product_run_retry", { projectId, request }),
  },
};
