import { beforeEach, describe, expect, expectTypeOf, it, vi } from "vitest";
import { productClient } from "./client";
import { normalizeProductError, ProductError } from "./errors";
import type { GeneratorOption, ProductRun, ProjectOverview } from "./types";
const { invokeCommand } = vi.hoisted(() => ({ invokeCommand: vi.fn() }));
vi.mock("../services/ipc", () => ({ invokeCommand }));

describe("product facade client", () => {
  it("enforces the existing architecture guard in ordinary frontend CI", async () => {
    const script = new URL("../../scripts/dev088-architecture-guard.mjs", import.meta.url).href;
    const guard = await import(/* @vite-ignore */ script);
    expect(guard.productBoundaryVerified).toBe(true);
  }, 15_000); // Whole-repo AST scan; CI measured ~5.6s, beyond Vitest's 5s default.
  beforeEach(() => { invokeCommand.mockReset(); });
  it("routes exactly five typed use cases through the single transport", async () => {
    invokeCommand.mockResolvedValue([]);
    const ref = { source: "queue-batch", id: "batch" } as const;
    await productClient.project.getOverview("project");
    await productClient.creation.generatorsList("project", "video");
    await productClient.project.generatorBindingSet("project", { stage: "VIDEO", mode: "DEFAULT", selectionRef: "opaque", expectedRevision: null, expectedBindingInstanceId: null });
    await productClient.run.get("project", ref);
    await productClient.run.retry("project", { ref, selectedItemIds: ["failed-source"] });
    expect(invokeCommand.mock.calls.map(call => call[0])).toEqual(["product_project_overview", "product_generators_list", "product_generator_binding_set", "product_run_get", "product_run_retry"]);
    expect(invokeCommand).toHaveBeenNthCalledWith(4, "product_run_get", { projectId: "project", runRef: ref });
    expect(invokeCommand).toHaveBeenNthCalledWith(5, "product_run_retry", { projectId: "project", request: { ref, selectedItemIds: ["failed-source"] } });
  });
  it("preserves conflict code/current state without raw diagnostic messages", async () => {
    const currentBinding = { stage: "VIDEO", mode: "DEFAULT", selectionRef: "opaque", revision: 2, bindingInstanceId: "instance" };
    invokeCommand.mockRejectedValue({ code: "GENERATOR_BINDING_CONFLICT", message: "CAS expectedRevision mismatch", details: { currentBinding, action: "REVIEW_CURRENT_BINDING", retryable: false, technicalDetails: "diagnostic" } });
    try { await productClient.project.getOverview("project"); expect.fail("expected conflict"); }
    catch (error) {
      expect(error).toBeInstanceOf(ProductError);
      if (!(error instanceof ProductError)) throw error;
      expect(error.code).toBe("GENERATOR_BINDING_CONFLICT");
      expect(error.message).not.toContain("CAS");
      expect(error.details.currentBinding).toEqual(currentBinding);
      expect(error.details.retryable).toBe(false);
    }
    expect(invokeCommand).toHaveBeenCalledTimes(1);
  });
  it("falls back safely for unknown errors", () => {
    const error = normalizeProductError({ code: "UNKNOWN_DATABASE_ERROR", message: "secret path" });
    expect(error.code).toBe("INTERNAL_ERROR"); expect(error.message).not.toContain("secret");
    expect(error.details.technicalDetails).toBeDefined();
  });
  it("exposes product types without raw generator identity", () => {
    expectTypeOf<keyof GeneratorOption>().exclude<"selectionRef" | "name" | "version" | "mode" | "mediaKind" | "availability" | "availabilityReason" | "recommended">().toEqualTypeOf<never>();
    expectTypeOf(productClient.project.getOverview).returns.resolves.toEqualTypeOf<ProjectOverview>();
    expectTypeOf(productClient.run.get).returns.resolves.toEqualTypeOf<ProductRun>();
  });
});
