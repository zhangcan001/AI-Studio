// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type { WorkflowSavedVersionDetailsView } from "../../types/workflowOnboarding";
import { WorkflowExecutionConfiguration } from "./WorkflowExecutionConfiguration";

const mocks = vi.hoisted(() => ({
  preflightWorkflowExecution: vi.fn(),
  createWorkflowExecution: vi.fn(),
  createWorkflowExecutionBatch: vi.fn(),
  startProductionQueue: vi.fn(),
  getProductionQueue: vi.fn(),
  getTaskDetail: vi.fn(),
  getWorkflowExecutionSummary: vi.fn(),
  getWorkflowRecipeHistory: vi.fn(),
}));
vi.mock("../../services/tauriClient", async () => ({
  ...await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient"),
  ...mocks,
}));

const recipe: RecipeViewModel = {
  workflowId: "wfl_test", workflowVersionId: "wfv_test", recipeId: "rcp_test",
  name: "Test", category: "image", mode: "text_to_image",
  fields: [{ key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" }],
};
const seedRecipe: RecipeViewModel = {
  ...recipe,
  fields: [
    { key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" },
    { key: "seed", type: "seed", label: "Seed", defaultMode: "fixed", defaultValue: "41" },
  ],
};
const details: WorkflowSavedVersionDetailsView = {
  workflowId: recipe.workflowId, workflowVersionId: recipe.workflowVersionId,
  workflowVersion: "1.0.0", name: "Test", category: "image", mode: "text_to_image",
  workflowSha256: "hash", workflowJson: {}, sourceWorkflowJson: {}, sourceWorkflowPreserved: true,
  recognition: {
    recognitionEngine: "WORKFLOW_RECOGNITION_V3", recognitionEngineVersion: "3",
    recognizedAt: "2026-09-26T00:00:00Z", sourceFormat: "API", schemaSource: "LIVE_COMFYUI",
    inferredType: "image", inferredMode: "text_to_image", finalType: "image", finalMode: "text_to_image",
    semanticCapabilityStatus: "READY", runtimeImportStatus: "READY", outputRootState: "READY",
    roots: [], evidenceSummary: [], issueCodes: [], runtimeBlockers: [],
    inputMappingDecisions: [{ semanticKey: "prompt", finalMapping: { nodeId: "1", inputName: "text" }, mappingSource: "MODEL_INFERRED" }],
  },
};

afterEach(cleanup);
beforeEach(() => {
  vi.clearAllMocks();
  mocks.preflightWorkflowExecution.mockResolvedValue({ status: "READY" });
  mocks.createWorkflowExecution.mockResolvedValue({ id: "pbt_test", status: "READY", items: [{ id: "pbi_test", status: "PENDING" }] });
  mocks.createWorkflowExecutionBatch.mockResolvedValue({ id: "pbt_batch", name: "Batch", status: "READY", total: 4, pending: 4, running: 0, succeeded: 0, failed: 0, cancelled: 0, skipped: 0, items: [] });
  mocks.startProductionQueue.mockResolvedValue(undefined);
  mocks.getProductionQueue.mockResolvedValue({ id: "pbt_test", status: "RUNNING", items: [{ id: "pbi_test", status: "DISPATCHED" }] });
  mocks.getWorkflowExecutionSummary.mockResolvedValue(null);
  mocks.getWorkflowRecipeHistory.mockResolvedValue({ taskPage: { items: [] } });
});

describe("saved version execution configuration", () => {
  it("opens offline without creating a queue job", async () => {
    render(<WorkflowExecutionConfiguration details={details} catalog={[recipe]} projectId="prj_test" comfyConnected={false} />);
    expect(screen.getByText(/COMFYUI_CONNECTION_UNAVAILABLE/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "运行工作流" }).hasAttribute("disabled")).toBe(true);
    expect(mocks.createWorkflowExecution).not.toHaveBeenCalled();
  });

  it("blocks an unresolved saved mapping before preflight or queue creation", async () => {
    render(<WorkflowExecutionConfiguration details={{ ...details, recognition: { ...details.recognition!, issueCodes: ["INPUT_MAPPING_UNRESOLVED"] } }} catalog={[recipe]} projectId="prj_test" comfyConnected />);
    expect(screen.getByText(/INPUT_MAPPING_UNRESOLVED/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "运行工作流" }).hasAttribute("disabled")).toBe(true);
    expect(mocks.preflightWorkflowExecution).not.toHaveBeenCalled();
  });

  it("lets a legacy saved recipe use its authoritative bindings without V3 provenance", async () => {
    render(<WorkflowExecutionConfiguration details={{ ...details, recognition: undefined }} catalog={[recipe]} projectId="prj_test" comfyConnected />);
    expect(screen.queryByText(/INPUT_MAPPING_UNRESOLVED/)).toBeNull();
    expect(screen.getByRole("button", { name: "运行工作流" }).hasAttribute("disabled")).toBe(false);
  });

  it("checks exact version and recipe with user values before queue creation", async () => {
    const user = userEvent.setup();
    render(<WorkflowExecutionConfiguration details={details} catalog={[recipe]} projectId="prj_test" comfyConnected />);
    await user.type(screen.getAllByRole("textbox")[1], "safe test");
    await user.click(screen.getByRole("button", { name: "运行工作流" }));
    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledWith("prj_test", "pbt_test"));
    expect(mocks.preflightWorkflowExecution).toHaveBeenCalledWith({
      projectId: "prj_test", workflowVersionId: "wfv_test", recipeId: "rcp_test",
      values: { prompt: { type: "string", value: "safe test" } },
    });
    expect(mocks.createWorkflowExecution).toHaveBeenCalledWith(expect.objectContaining({
      inputSources: { prompt: "USER_INPUT" },
    }));
    expect(mocks.preflightWorkflowExecution.mock.invocationCallOrder[0]).toBeLessThan(mocks.createWorkflowExecution.mock.invocationCallOrder[0]);
  });

  it("does not create a queue job when runtime preflight blocks", async () => {
    mocks.preflightWorkflowExecution.mockResolvedValue({ status: "BLOCKED", code: "COMFYUI_CONNECTION_UNAVAILABLE" });
    const user = userEvent.setup();
    render(<WorkflowExecutionConfiguration details={details} catalog={[recipe]} projectId="prj_test" comfyConnected />);
    await user.type(screen.getAllByRole("textbox")[1], "safe test");
    await user.click(screen.getByRole("button", { name: "运行工作流" }));
    await waitFor(() => expect(screen.getByText(/Runtime Preflight: BLOCKED/)).toBeTruthy());
    expect(mocks.createWorkflowExecution).not.toHaveBeenCalled();
  });

  it("previews and persists a prompt-by-seed batch through the existing queue path", async () => {
    const user = userEvent.setup();
    render(<WorkflowExecutionConfiguration details={details} catalog={[seedRecipe]} projectId="prj_test" comfyConnected />);
    fireEvent.click(screen.getByText("批量生成"));
    fireEvent.change(screen.getByRole("textbox", { name: "Prompt 列表（每行一个）" }), { target: { value: "first prompt\nsecond prompt" } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Seed 数量" }), { target: { value: "2" } });

    expect(screen.getByText("将创建 4 个任务")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "创建并启动批次" }));
    await waitFor(() => expect(mocks.startProductionQueue).toHaveBeenCalledWith("prj_test", "pbt_batch"));
    expect(mocks.createWorkflowExecutionBatch).toHaveBeenCalledWith(expect.objectContaining({
      projectId: "prj_test",
      workflowVersionId: "wfv_test",
      recipeId: "rcp_test",
      items: [
        expect.objectContaining({ values: { prompt: { type: "string", value: "first prompt" }, seed: { type: "seed_fixed", value: "41" } } }),
        expect.objectContaining({ values: { prompt: { type: "string", value: "first prompt" }, seed: { type: "seed_fixed", value: "42" } } }),
        expect.objectContaining({ values: { prompt: { type: "string", value: "second prompt" }, seed: { type: "seed_fixed", value: "41" } } }),
        expect.objectContaining({ values: { prompt: { type: "string", value: "second prompt" }, seed: { type: "seed_fixed", value: "42" } } }),
      ],
    }));
    expect(mocks.preflightWorkflowExecution).not.toHaveBeenCalled();
  });

  it("blocks an over-limit prompt and seed product before queue creation", () => {
    render(<WorkflowExecutionConfiguration details={details} catalog={[seedRecipe]} projectId="prj_test" comfyConnected />);
    fireEvent.click(screen.getByText("批量生成"));
    const prompts = Array.from({ length: 51 }, (_, index) => `prompt ${index + 1}`).join("\n");
    fireEvent.change(screen.getByRole("textbox", { name: "Prompt 列表（每行一个）" }), { target: { value: prompts } });
    fireEvent.change(screen.getByRole("spinbutton", { name: "Seed 数量" }), { target: { value: "2" } });
    expect(screen.getByText("将创建 102 个任务")).toBeTruthy();
    expect(screen.getByRole("button", { name: "创建并启动批次" }).hasAttribute("disabled")).toBe(true);
    expect(mocks.createWorkflowExecutionBatch).not.toHaveBeenCalled();
  });

  it("renders a reopened synthetic 100-item batch from persisted queue state", async () => {
    const prompts = Array.from({ length: 100 }, (_, index) => `prompt ${index + 1}`);
    const items = prompts.map((promptText, ordinal) => ({
      id: `pbi_${ordinal + 1}`,
      ordinal,
      status: "PENDING",
      promptText,
      seed: String(41 + ordinal),
      inputAssetIds: [],
    }));
    const persistedBatch = {
      id: "pbt_synthetic_100",
      name: "Synthetic 100",
      status: "READY",
      total: 100,
      pending: 100,
      running: 0,
      succeeded: 0,
      failed: 0,
      cancelled: 0,
      skipped: 0,
      items,
    };
    mocks.createWorkflowExecutionBatch.mockResolvedValue(persistedBatch);
    mocks.getProductionQueue.mockResolvedValue(persistedBatch);
    const user = userEvent.setup();
    const { container } = render(<WorkflowExecutionConfiguration details={details} catalog={[seedRecipe]} projectId="prj_test" comfyConnected />);
    fireEvent.click(screen.getByText("批量生成"));
    fireEvent.change(screen.getByRole("textbox", { name: "Prompt 列表（每行一个）" }), { target: { value: prompts.join("\n") } });

    expect(screen.getByText("将创建 100 个任务")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "创建并启动批次" }));
    await waitFor(() => expect(mocks.getProductionQueue).toHaveBeenCalledWith("prj_test", "pbt_synthetic_100"));
    await waitFor(() => expect(container.querySelectorAll(".workflow-execution-batch-item")).toHaveLength(100));
    expect(screen.getByText(/任务 100/)).toBeTruthy();
    expect(screen.getByText(/总计 100 · 等待 100/)).toBeTruthy();
  });

  it("reopens a persisted result for the exact recipe and project", async () => {
    mocks.getWorkflowRecipeHistory.mockResolvedValue({ taskPage: { items: [{ id: "tsk_previous", projectId: "prj_test", status: "SUCCEEDED" }] } });
    mocks.getTaskDetail.mockResolvedValue({ id: "tsk_previous", status: "SUCCEEDED", outputAssets: [{ id: "ast_result", name: "result.png" }] });
    mocks.getWorkflowExecutionSummary.mockResolvedValue({
      workflowId: "wfl_test", workflowVersionId: "wfv_test", workflowVersion: "1.0.0",
      recipeId: "rcp_test", recipeVersion: "1.0.0", createdAt: "2026-09-27T00:00:00Z",
      runtimeSchemaSource: "LIVE_COMFYUI", preflightStatus: "READY",
      inputs: [{ semanticField: "prompt", targets: [{ node: "1", input: "text" }], valueType: "string", valueSummary: "saved safe test", source: "USER_INPUT" }],
    });
    render(<WorkflowExecutionConfiguration details={details} catalog={[recipe]} projectId="prj_test" comfyConnected />);
    await waitFor(() => expect(screen.getByText(/历史执行 tsk_previous/)).toBeTruthy());
    expect(mocks.getWorkflowRecipeHistory).toHaveBeenCalledWith(
      "wfv_test",
      "rcp_test",
      undefined,
      20,
      "prj_test",
      ["SUCCEEDED", "FAILED"],
    );
    expect(mocks.getTaskDetail).toHaveBeenCalledWith("prj_test", "tsk_previous");
    expect(mocks.getWorkflowExecutionSummary).toHaveBeenCalledWith("prj_test", "tsk_previous");
    expect(screen.getByText(/ast_result · result.png · AI_STUDIO_MANAGED/)).toBeTruthy();
    expect(screen.getByText(/saved safe test · USER_INPUT/)).toBeTruthy();
  });

  it("reopens a persisted failure and shows only its safe error-code summary", async () => {
    mocks.getWorkflowRecipeHistory.mockResolvedValue({ taskPage: { items: [{ id: "tsk_failed", projectId: "prj_test", status: "FAILED" }] } });
    mocks.getTaskDetail.mockResolvedValue({
      id: "tsk_failed", status: "FAILED", errorCode: "WORKFLOW_VALIDATION_FAILED",
      errorMessage: "unsafe raw details must not be rendered", outputAssets: [],
    });
    mocks.getWorkflowExecutionSummary.mockResolvedValue({
      workflowId: "wfl_test", workflowVersionId: "wfv_test", workflowVersion: "1.0.0",
      recipeId: "rcp_test", recipeVersion: "1.0.0", createdAt: "2026-09-27T00:00:00Z",
      runtimeSchemaSource: "LIVE_COMFYUI", preflightStatus: "READY",
      inputs: [{ semanticField: "prompt", targets: [{ node: "1", input: "text" }], valueType: "string", valueSummary: "saved safe test", source: "USER_INPUT" }],
    });

    render(<WorkflowExecutionConfiguration details={details} catalog={[recipe]} projectId="prj_test" comfyConnected />);
    await waitFor(() => expect(screen.getByText(/历史执行 tsk_failed · FAILED/)).toBeTruthy());

    expect(screen.getByText(/WORKFLOW_VALIDATION_FAILED · 工作流校验未通过/)).toBeTruthy();
    expect(screen.getByText(/saved safe test · USER_INPUT/)).toBeTruthy();
    expect(screen.queryByText(/unsafe raw details/)).toBeNull();
    expect(mocks.getWorkflowExecutionSummary).toHaveBeenCalledWith("prj_test", "tsk_failed");
  });

  it("reconciles a completed batch, loads skipped terminal details, and reruns with a new key", async () => {
    const user = userEvent.setup();
    const completedBatch = {
      id: "pbt_test",
      name: "Completed",
      status: "COMPLETED",
      total: 1,
      pending: 0,
      running: 0,
      succeeded: 0,
      failed: 0,
      cancelled: 0,
      skipped: 1,
      items: [{ id: "pbi_test", ordinal: 0, status: "SKIPPED", taskId: "tsk_skipped", inputAssetIds: [] }],
    };
    mocks.createWorkflowExecution.mockResolvedValue({ ...completedBatch, status: "READY" });
    mocks.getProductionQueue.mockResolvedValue(completedBatch);
    mocks.getTaskDetail.mockResolvedValue({ id: "tsk_skipped", status: "SKIPPED", outputAssets: [] });

    render(<WorkflowExecutionConfiguration details={details} catalog={[recipe]} projectId="prj_test" comfyConnected />);
    await user.type(screen.getAllByRole("textbox")[1], "rerun-safe");
    await user.click(screen.getByRole("button", { name: "运行工作流" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "再次执行" })).toBeTruthy());
    expect(mocks.getTaskDetail).toHaveBeenCalledWith("prj_test", "tsk_skipped");

    const firstKey = mocks.createWorkflowExecution.mock.calls[0][0].submissionIdempotencyKey;
    await user.click(screen.getByRole("button", { name: "再次执行" }));
    await user.click(screen.getByRole("button", { name: "运行工作流" }));
    await waitFor(() => expect(mocks.createWorkflowExecution).toHaveBeenCalledTimes(2));
    const secondKey = mocks.createWorkflowExecution.mock.calls[1][0].submissionIdempotencyKey;
    expect(secondKey).not.toBe(firstKey);
    expect(mocks.createWorkflowExecution.mock.calls[1][0].values).toEqual({
      prompt: { type: "string", value: "rerun-safe" },
    });
  });
});
