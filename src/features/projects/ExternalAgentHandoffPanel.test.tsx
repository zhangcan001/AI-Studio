// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  confirmExternalProductionHandoff,
  listExternalProductionHandoffs,
  previewExternalProductionHandoff,
} from "../../services/tauriClient";
import { ExternalAgentHandoffPanel } from "./ExternalAgentHandoffPanel";

vi.mock("../../services/tauriClient", () => ({
  confirmExternalProductionHandoff: vi.fn(),
  getExternalProductionHandoffMappings: vi.fn(),
  listExternalProductionHandoffs: vi.fn(),
  previewExternalProductionHandoff: vi.fn(),
}));

const validPreview = (overrides: Record<string, unknown> = {}) => ({
  projectId: "project-1",
  documentSha256: "a".repeat(64),
  seriesCount: 1,
  episodeCount: 1,
  sceneCount: 1,
  shotCount: 1,
  normalized: { schemaVersion: 1 as const, projectId: "project-1", source: { agent: "test" }, series: [] },
  errors: [],
  warnings: [],
  replay: { status: "NEW", mappings: [] },
  writePlan: {
    createsSeries: 1,
    createsEpisodes: 1,
    createsScenes: 1,
    createsShots: 1,
    writesPrompts: 1,
    writesAssetReferences: 0,
    writesStageConfigs: 0,
    createsProvenanceMappings: 4,
  },
  ...overrides,
});

describe("ExternalAgentHandoffPanel", () => {
  beforeEach(() => {
    vi.mocked(listExternalProductionHandoffs).mockResolvedValue([]);
    vi.mocked(previewExternalProductionHandoff).mockReset();
    vi.mocked(confirmExternalProductionHandoff).mockReset();
    vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("previews and confirms a valid handoff through the typed client", async () => {
    vi.mocked(previewExternalProductionHandoff).mockResolvedValue(validPreview());
    vi.mocked(confirmExternalProductionHandoff).mockResolvedValue({
      projectId: "project-1",
      handoffId: "hnd-1",
      documentSha256: "a".repeat(64),
      replayed: false,
      seriesCount: 1,
      episodeCount: 1,
      sceneCount: 1,
      shotCount: 1,
      mappings: [],
    });
    const onImported = vi.fn();
    const onOpenStructure = vi.fn();
    const actor = userEvent.setup();
    render(<ExternalAgentHandoffPanel projectId="project-1" onBack={vi.fn()} onImported={onImported} onOpenStructure={onOpenStructure} />);

    fireEvent.change(screen.getByLabelText("或粘贴交接 JSON"), { target: { value: '{"schemaVersion":1}' } });
    await actor.click(screen.getByRole("button", { name: "运行交接预检" }));
    await waitFor(() => expect(previewExternalProductionHandoff).toHaveBeenCalledWith({
      projectId: "project-1",
      content: '{"schemaVersion":1}',
    }));
    await actor.click(screen.getByRole("button", { name: "明确确认并写入" }));
    await waitFor(() => expect(confirmExternalProductionHandoff).toHaveBeenCalledWith({
      projectId: "project-1",
      content: '{"schemaVersion":1}',
      expectedDocumentSha256: "a".repeat(64),
    }));
    expect(onImported).toHaveBeenCalledTimes(1);
    await actor.click(screen.getByRole("button", { name: "打开项目结构" }));
    expect(onOpenStructure).toHaveBeenCalledTimes(1);
  });

  it("keeps confirmation blocked for schema errors and exposes stale confirmation failures", async () => {
    vi.mocked(previewExternalProductionHandoff).mockResolvedValue(validPreview({
      errors: [{ severity: "error", code: "HANDOFF_UNKNOWN_FIELD", message: "unknown field" }],
    }));
    const actor = userEvent.setup();
    render(<ExternalAgentHandoffPanel projectId="project-1" onBack={vi.fn()} />);
    fireEvent.change(screen.getByLabelText("或粘贴交接 JSON"), { target: { value: "{}" } });
    await actor.click(screen.getByRole("button", { name: "运行交接预检" }));
    await waitFor(() => expect(screen.getByText("HANDOFF_UNKNOWN_FIELD")).toBeTruthy());
    expect(screen.getByRole("button", { name: "明确确认并写入" })).toHaveProperty("disabled", true);
  });

  it("reloads project-scoped history and clears the handoff draft when project changes", async () => {
    const { rerender } = render(<ExternalAgentHandoffPanel projectId="project-1" onBack={vi.fn()} />);
    await waitFor(() => expect(listExternalProductionHandoffs).toHaveBeenCalledWith("project-1"));
    const actor = userEvent.setup();
    await actor.type(screen.getByLabelText("或粘贴交接 JSON"), "draft");
    rerender(<ExternalAgentHandoffPanel projectId="project-2" onBack={vi.fn()} />);
    await waitFor(() => expect(listExternalProductionHandoffs).toHaveBeenCalledWith("project-2"));
    expect(screen.getByLabelText("或粘贴交接 JSON")).toHaveProperty("value", "");
  });
});
