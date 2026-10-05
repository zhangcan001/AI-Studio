import { beforeEach, describe, expect, expectTypeOf, it, vi } from "vitest";
import { productClient } from "./client";
import { normalizeProductError, ProductError } from "./errors";
import type { GeneratorOption, ProductRun, ProjectOverview } from "./types";
const { invokeCommand } = vi.hoisted(() => ({ invokeCommand: vi.fn() }));
vi.mock("../services/ipc", () => ({ invokeCommand }));

describe("product facade client", () => {
  it("M3-1 read-only typed tag adapter and combined filters reach Product transport",async()=>{
    const tags=[{id:"tag-a",projectId:"project-a",name:"Tag A",createdAt:"date",updatedAt:"date"}];
    invokeCommand.mockResolvedValueOnce(tags);
    expect(await productClient.library.tagsList("project-a")).toEqual(tags);
    expect(invokeCommand).toHaveBeenLastCalledWith("product_library_tags_list",{projectId:"project-a"});
    const query={category:"images",keyword:"later",favoriteOnly:true,tagId:"tag-a",cursor:null,limit:30} as const;
    invokeCommand.mockResolvedValueOnce({items:[]});await productClient.library.list("project-a",query);
    expect(invokeCommand).toHaveBeenLastCalledWith("product_library_list",{projectId:"project-a",query});
  });
  it("phase5 target8 transports typed library details and preserves missing-resource errors", async () => {
    const query = { category: "prompts", keyword: "中文", cursor: null, limit: 20 } as const;
    const page = { items: [], nextCursor: null, coverage: "keyset-page", coverageMessage: "数据库分页" };
    invokeCommand.mockResolvedValueOnce(page);
    expect(await productClient.library.list("project-a", query)).toEqual(page);
    for (const kind of ["asset", "prompt", "profile", "reference-set"] as const) {
      const ref = { kind, id: `${kind}-owned` };
      invokeCommand.mockResolvedValueOnce({ kind });
      await productClient.library.get("project-a", ref);
      expect(invokeCommand).toHaveBeenLastCalledWith("product_library_get", { projectId: "project-a", resource: ref });
    }
    invokeCommand.mockRejectedValueOnce({ code: "LIBRARY_RESOURCE_NOT_FOUND", message: "secret db path" });
    await expect(productClient.library.get("project-b", { kind: "asset", id: "missing" })).rejects.toMatchObject({ code: "LIBRARY_RESOURCE_NOT_FOUND", message: "资源已删除或不属于当前项目，请返回资源库。" });
    const ref = {kind:"asset",id:"owned"} as const;
    invokeCommand.mockResolvedValue(undefined);
    for(const [call,command] of [[()=>productClient.library.mediaVerify("project-a",ref),"product_library_media_verify"],[()=>productClient.library.thumbnailGet("project-a",ref),"product_library_thumbnail_get"],[()=>productClient.library.imageGet("project-a",ref),"product_library_image_get"],[()=>productClient.library.relationsGet("project-a",ref),"product_library_relations_get"],[()=>productClient.library.versionsGet("project-a",ref),"product_library_versions_get"],[()=>productClient.library.useInCreation("project-a",ref),"product_library_use_in_creation"],[()=>productClient.library.deletionInspect("project-a",ref),"product_library_deletion_inspect"]] as const){await call();expect(invokeCommand).toHaveBeenLastCalledWith(command,{projectId:"project-a",resource:ref});}
    await productClient.library.delete("project-a",ref,true);expect(invokeCommand).toHaveBeenLastCalledWith("product_library_delete",{projectId:"project-a",resource:ref,confirmed:true});
    const edit={kind:"prompt",id:"prompt-a",text:"new",modelVersionId:null} as const;
    await productClient.library.resourceEdit("project-a",edit);expect(invokeCommand).toHaveBeenLastCalledWith("product_library_resource_edit",{projectId:"project-a",request:edit});
    expectTypeOf(productClient.library.get).returns.resolves.toEqualTypeOf<import("./libraryTypes").LibraryDetail>();
  });
  it("enforces the existing architecture guard in ordinary frontend CI", async () => {
    const script = new URL("../../scripts/dev088-architecture-guard.mjs", import.meta.url).href;
    const guard = await import(/* @vite-ignore */ script);
    expect(guard.productBoundaryVerified).toBe(true);
  }, 30_000); // M2-2 full guard finished in 18.05s on CI (local ~10s); finite verification budget, not a product performance threshold.
  beforeEach(() => { invokeCommand.mockReset(); });
  it("routes the five original typed use cases through the single transport", async () => {
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
  it("routes creation context and mutations through the product transport", async () => {
    invokeCommand.mockResolvedValue(undefined);
    await productClient.creation.get("project", null, "image");
    await productClient.creation.createShot("project");
    await productClient.creation.updateShot("project", { shotId: "shot", name: "镜头二" });
    await productClient.creation.deleteShot("project", "shot");
    await productClient.creation.referencesSet("project", "shot", "video", ["first", "last"]);
    await productClient.creation.selectResult("project", "shot", "image", "candidate");
    expect(invokeCommand.mock.calls).toEqual([
      ["product_creation_get", { projectId: "project", shotId: null, stage: "image" }],
      ["product_creation_shot_create", { projectId: "project" }],
      ["product_creation_shot_update", { projectId: "project", request: { shotId: "shot", name: "镜头二" } }],
      ["product_creation_shot_delete", { projectId: "project", shotId: "shot" }],
      ["product_creation_references_set", { projectId: "project", shotId: "shot", stage: "video", assetIds: ["first", "last"] }],
      ["product_creation_result_select", { projectId: "project", shotId: "shot", stage: "image", assetId: "candidate" }],
    ]);
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
  it("sends the same typed draft to readiness and generate without persisting media references", async () => {
    const request = { projectId: "project", shotId: "shot", stage: "video", selectionRef: "opaque",
      values: { reference_video: { type: "video_asset", assetId: "video" }, prompt: { type: "string", value: "draft" } },
      submissionIdempotencyKey: "same-attempt" } as const;
    invokeCommand.mockResolvedValueOnce({ ready: true, issues: [], fieldErrors: [], actions: [] });
    const readiness = await productClient.creation.readinessGet(request);
    expect(readiness.ready).toBe(true);
    const accepted = { accepted: true, runRef: { source: "queue-batch", id: "persisted" }, startOutcome: "FAILED_TO_START", startIssue: null };
    invokeCommand.mockResolvedValueOnce(accepted);
    expect(await productClient.creation.generate(request)).toEqual(accepted);
    expect(invokeCommand.mock.calls).toEqual([
      ["product_creation_readiness_get", { request }], ["product_creation_generate", { request }],
    ]);
    expect(accepted).not.toHaveProperty("success");
  });
  it("keeps typed readiness fields and hides raw runtime diagnostics", () => {
    for (const code of ["MISSING_INPUT", "INPUT_OUT_OF_RANGE", "ASSET_TYPE_MISMATCH", "ASSET_PROJECT_MISMATCH", "RUNTIME_BLOCKED"] as const) {
      const error = normalizeProductError({ code, message: "node=secret input=secret", details: { field: "reference_video", action: "EDIT_INPUT", technicalDetails: "diagnostic" } });
      expect(error.code).toBe(code);
      expect(error.details.field).toBe("reference_video");
      expect(error.message).not.toContain("secret");
    }
  });
  it("falls back safely for unknown errors", () => {
    const error = normalizeProductError({ code: "UNKNOWN_DATABASE_ERROR", message: "secret path" });
    expect(error.code).toBe("INTERNAL_ERROR"); expect(error.message).not.toContain("secret");
    expect(error.details.technicalDetails).toBeDefined();
  });
  it("exposes product types without raw generator identity", () => {
    expectTypeOf<keyof GeneratorOption>().exclude<"selectionRef" | "name" | "version" | "mode" | "mediaKind" | "availability" | "availabilityReason" | "recommended" | "fields">().toEqualTypeOf<never>();
    expectTypeOf(productClient.project.getOverview).returns.resolves.toEqualTypeOf<ProjectOverview>();
    expectTypeOf(productClient.run.get).returns.resolves.toEqualTypeOf<ProductRun>();
  });
});
