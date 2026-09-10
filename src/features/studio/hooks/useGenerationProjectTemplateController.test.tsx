// @vitest-environment jsdom

import { act, cleanup, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { GenerationValues, RecipeViewModel } from "../../../types/generation";
import type { ProjectTemplate } from "../../../types/organization";
import { useGenerationProjectTemplateController } from "./useGenerationProjectTemplateController";

const mocks = vi.hoisted(() => ({
  createProjectTemplate: vi.fn(),
}));

vi.mock("../../../services/tauriClient", async () => {
  const actual = await vi.importActual<typeof import("../../../services/tauriClient")>("../../../services/tauriClient");
  return {
    ...actual,
    createProjectTemplate: mocks.createProjectTemplate,
  };
});

const workflow: RecipeViewModel = {
  workflowId: "workflow-a",
  workflowVersionId: "workflow-version-a",
  recipeId: "recipe-a",
  name: "Workflow A",
  category: "image",
  mode: "text-to-image",
  fields: [{ key: "prompt", type: "textarea", label: "Prompt", required: true, default: "" }],
};

const values: GenerationValues = {
  prompt: { type: "string", value: "template prompt" },
  seed: { type: "seed_fixed", value: "42" },
};

const createdTemplate: ProjectTemplate = {
  id: "template-a",
  name: "Template A",
  description: "Description",
  workflowVersionId: workflow.workflowVersionId,
  recipeId: workflow.recipeId,
  values,
  available: true,
  createdAt: "2026-09-10T00:00:00Z",
  updatedAt: "2026-09-10T00:00:00Z",
};

function options(overrides: Partial<Parameters<typeof useGenerationProjectTemplateController>[0]> = {}) {
  return {
    projectId: "project-a",
    selectedWorkflow: workflow,
    values,
    onNotice: vi.fn(),
    ...overrides,
  };
}

describe("useGenerationProjectTemplateController", () => {
  beforeEach(() => {
    mocks.createProjectTemplate.mockReset();
    mocks.createProjectTemplate.mockResolvedValue(createdTemplate);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("starts closed, opens, edits, and closes without changing entered fields", () => {
    const { result } = renderHook(() => useGenerationProjectTemplateController(options()));

    expect(result.current.templateEditorOpen).toBe(false);
    expect(result.current.templateName).toBe("");
    expect(result.current.templateDescription).toBe("");
    expect(result.current.templateSaving).toBe(false);
    expect(result.current.templateError).toBeUndefined();

    act(() => {
      result.current.openEditor();
      result.current.setTemplateName("Template A");
      result.current.setTemplateDescription("Description");
    });
    expect(result.current.templateEditorOpen).toBe(true);
    expect(result.current.templateName).toBe("Template A");
    expect(result.current.templateDescription).toBe("Description");

    act(() => result.current.closeEditor());
    expect(result.current.templateEditorOpen).toBe(false);
    expect(result.current.templateName).toBe("Template A");
    expect(result.current.templateDescription).toBe("Description");
  });

  it("does not save without a selected workflow or a non-empty name", async () => {
    const missingWorkflow = renderHook(() => useGenerationProjectTemplateController(options({ selectedWorkflow: undefined })));
    act(() => {
      missingWorkflow.result.current.openEditor();
      missingWorkflow.result.current.setTemplateName("Template A");
    });
    await act(async () => { await missingWorkflow.result.current.save(); });
    expect(mocks.createProjectTemplate).not.toHaveBeenCalled();

    const emptyName = renderHook(() => useGenerationProjectTemplateController(options()));
    act(() => emptyName.result.current.openEditor());
    await act(async () => { await emptyName.result.current.save(); });
    expect(mocks.createProjectTemplate).not.toHaveBeenCalled();
  });

  it("preserves the exact identity and values while trimming an optional description", async () => {
    const onNotice = vi.fn();
    const { result } = renderHook(() => useGenerationProjectTemplateController(options({ onNotice })));

    act(() => {
      result.current.openEditor();
      result.current.setTemplateName("  Template A  ");
      result.current.setTemplateDescription("  Description  ");
    });
    await act(async () => { await result.current.save(); });

    expect(mocks.createProjectTemplate).toHaveBeenCalledWith({
      name: "  Template A  ",
      description: "Description",
      workflowVersionId: "workflow-version-a",
      recipeId: "recipe-a",
      values,
    });
    expect(onNotice).toHaveBeenCalledWith("项目模板已保存；素材输入不会写入模板。");
    expect(result.current.templateEditorOpen).toBe(false);
    expect(result.current.templateName).toBe("");
    expect(result.current.templateDescription).toBe("");
    expect(result.current.templateSaving).toBe(false);
  });

  it("converts an empty description to undefined", async () => {
    const { result } = renderHook(() => useGenerationProjectTemplateController(options()));
    act(() => {
      result.current.openEditor();
      result.current.setTemplateName("Template A");
      result.current.setTemplateDescription("  ");
    });

    await act(async () => { await result.current.save(); });

    expect(mocks.createProjectTemplate).toHaveBeenCalledWith(expect.objectContaining({ description: undefined }));
  });

  it("keeps the editor usable and preserves entered content when save fails", async () => {
    mocks.createProjectTemplate.mockRejectedValueOnce(new Error("save failed"));
    const { result } = renderHook(() => useGenerationProjectTemplateController(options()));
    act(() => {
      result.current.openEditor();
      result.current.setTemplateName("Template A");
      result.current.setTemplateDescription("Description");
    });

    await act(async () => { await result.current.save(); });

    expect(result.current.templateEditorOpen).toBe(true);
    expect(result.current.templateName).toBe("Template A");
    expect(result.current.templateDescription).toBe("Description");
    expect(result.current.templateSaving).toBe(false);
    expect(result.current.templateError).toBe("操作失败，请查看技术详情。");
  });

  it("keeps saving state and blocks a second submit until the first save finishes", async () => {
    let resolveSave!: (template: ProjectTemplate) => void;
    mocks.createProjectTemplate.mockImplementationOnce(() => new Promise<ProjectTemplate>((resolve) => {
      resolveSave = resolve;
    }));
    const { result } = renderHook(() => useGenerationProjectTemplateController(options()));
    act(() => {
      result.current.openEditor();
      result.current.setTemplateName("Template A");
    });

    let firstSave!: Promise<void>;
    await act(async () => {
      firstSave = result.current.save();
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.templateSaving).toBe(true));
    await act(async () => { await result.current.save(); });
    expect(mocks.createProjectTemplate).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveSave(createdTemplate);
      await firstSave;
    });
    expect(result.current.templateSaving).toBe(false);
    expect(result.current.templateEditorOpen).toBe(false);
  });

  it("resets transient state on project change and ignores the old save completion", async () => {
    let resolveSave!: (template: ProjectTemplate) => void;
    mocks.createProjectTemplate.mockImplementationOnce(() => new Promise<ProjectTemplate>((resolve) => {
      resolveSave = resolve;
    }));
    const onNotice = vi.fn();
    const { result, rerender } = renderHook(
      (props: Parameters<typeof useGenerationProjectTemplateController>[0]) => useGenerationProjectTemplateController(props),
      { initialProps: options({ onNotice }) },
    );
    act(() => {
      result.current.openEditor();
      result.current.setTemplateName("Template A");
      result.current.setTemplateDescription("Description");
    });

    let oldSave!: Promise<void>;
    await act(async () => {
      oldSave = result.current.save();
      await Promise.resolve();
    });
    await waitFor(() => expect(result.current.templateSaving).toBe(true));

    rerender(options({ projectId: "project-b", onNotice }));
    await waitFor(() => {
      expect(result.current.templateEditorOpen).toBe(false);
      expect(result.current.templateName).toBe("");
      expect(result.current.templateDescription).toBe("");
      expect(result.current.templateSaving).toBe(false);
      expect(result.current.templateError).toBeUndefined();
    });

    await act(async () => {
      resolveSave(createdTemplate);
      await oldSave;
    });
    expect(result.current.templateEditorOpen).toBe(false);
    expect(result.current.templateName).toBe("");
    expect(onNotice).not.toHaveBeenCalled();
  });
});
