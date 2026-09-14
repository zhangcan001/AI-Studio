// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ProjectWorkspace } from "./ProjectWorkspace";
import type { ProjectBackupPreview, ProjectView } from "../../types/project";

vi.mock("../../services/tauriClient", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../../services/tauriClient")>();
  return {
    ...actual,
    createProject: vi.fn(),
    exportProjectBackup: vi.fn(),
    inspectProjectBackup: vi.fn(),
    restoreProjectBackup: vi.fn(),
    updateProject: vi.fn(),
    createProjectFromTemplate: vi.fn(),
    deleteProjectTemplate: vi.fn(),
    listProjectTemplates: vi.fn(async () => []),
    updateProjectTemplate: vi.fn(),
    getProjectWorkflowConfig: vi.fn(async () => ({
      projectId: "project-1",
      videoModeOverrides: [],
    })),
    replaceProjectWorkflowConfig: vi.fn(),
    getComfyPreflight: vi.fn(),
  };
});

vi.mock("./ProjectWorkflowSettings", () => ({
  ProjectWorkflowSettings: () => <div data-testid="workflow-settings-stub" />,
}));
vi.mock("./ProjectWorkflowPreflight", () => ({
  ProjectWorkflowPreflight: () => <div data-testid="workflow-preflight-stub" />,
}));
vi.mock("./ProjectProductionReadiness", () => ({
  ProjectProductionReadiness: () => <div data-testid="production-readiness-stub" />,
}));

import {
  exportProjectBackup,
  inspectProjectBackup,
  restoreProjectBackup,
} from "../../services/tauriClient";

const project: ProjectView = {
  id: "project-1",
  name: "测试项目",
  description: null,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

const preview: ProjectBackupPreview = {
  inspectionId: "bki_1",
  projectName: "归档项目",
  imageCount: 2,
  videoCount: 1,
  audioCount: 0,
  historyTasks: 3,
  presets: 1,
  productionQueues: 1,
  benchmarks: 0,
  productionRuns: 0,
  promptEntries: 2,
  shots: 4,
  assetVersions: 5,
  assetRelations: 2,
  models: 1,
  modelVersions: 1,
  tools: 1,
  toolInstances: 1,
  generationToolUsages: 3,
  generationAssetVersions: 4,
  missingWorkflows: [],
  activeTasksExcluded: 0,
  warning: "检查不会修改数据库。",
};

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  vi.unstubAllGlobals();
});

function renderWorkspace() {
  return render(
    <ProjectWorkspace
      projects={[project]}
      activeProjectId={project.id}
      catalog={[]}
      onOpen={vi.fn()}
      onProjectUpdated={vi.fn()}
      onProjectRestored={vi.fn()}
      onTemplateProjectCreated={vi.fn()}
    />,
  );
}

describe("ProjectWorkspace archive UX", () => {
  it("uses inspect-first archive labels and shows v2 preview counts", async () => {
    const user = userEvent.setup();
    vi.mocked(inspectProjectBackup).mockResolvedValue(preview);
    renderWorkspace();

    expect(screen.getAllByRole("button", { name: "导出归档" }).length).toBeGreaterThan(0);
    expect(screen.getByRole("button", { name: "检查归档" })).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "检查归档" }));

    expect(await screen.findByText("归档检查")).toBeTruthy();
    expect(screen.getByText(/先检查再恢复/)).toBeTruthy();
    expect(screen.getByText(/资产版本 5/)).toBeTruthy();
    expect(screen.getByText(/模型 1/)).toBeTruthy();
    expect(screen.getByText(/工具 1/)).toBeTruthy();
    expect(screen.getByText(/溯源 7/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "确认恢复（新建项目）" })).toBeTruthy();
  });

  it("restore confirm states new project, no overwrite, no auto-generation", async () => {
    const user = userEvent.setup();
    vi.mocked(inspectProjectBackup).mockResolvedValue(preview);
    vi.mocked(restoreProjectBackup).mockResolvedValue({
      ...project,
      id: "restored-1",
      name: "归档项目（恢复）",
    });
    const confirm = vi.fn(() => true);
    vi.stubGlobal("confirm", confirm);
    renderWorkspace();

    await user.click(screen.getByRole("button", { name: "检查归档" }));
    await screen.findByText("归档检查");
    await user.click(screen.getByRole("button", { name: "确认恢复（新建项目）" }));

    expect(confirm).toHaveBeenCalled();
    const message = String(confirm.mock.calls[0]?.[0] ?? "");
    expect(message).toContain("会新建项目");
    expect(message).toContain("不会覆盖");
    expect(message).toContain("不会自动生成");
    expect(restoreProjectBackup).toHaveBeenCalledWith("bki_1");
  });

  it("export uses typed transport only", async () => {
    const user = userEvent.setup();
    vi.mocked(exportProjectBackup).mockResolvedValue({
      fileName: "AI-Studio-Project.aiarchive",
      bytes: 128,
      entries: 8,
      activeTasksExcluded: 0,
    });
    renderWorkspace();
    await user.click(screen.getAllByRole("button", { name: "导出归档" })[0]!);
    expect(exportProjectBackup).toHaveBeenCalledWith("project-1");
    expect(await screen.findByText(/项目归档已保存：AI-Studio-Project.aiarchive/)).toBeTruthy();
  });
});
