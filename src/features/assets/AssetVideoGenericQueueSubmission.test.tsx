// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import { AssetVideoBatchWorkspace } from "./AssetVideoBatchWorkspace";
const mocks = vi.hoisted(() => ({ getProjectWorkflowConfig: vi.fn(), assetLibraryPage: vi.fn(), listAssetVideoPrompts: vi.fn(), listPresets: vi.fn(), getPreferredPreset: vi.fn(), listModels: vi.fn(), listModelVersions: vi.fn(), submitGeneration: vi.fn(), startProductionQueue: vi.fn(), createProductionQueue: vi.fn() }));
vi.mock("../../services/tauriClient", async () => ({ ...(await vi.importActual<typeof import("../../services/tauriClient")>("../../services/tauriClient")), ...mocks }));
const recipe: RecipeViewModel = { workflowId: "WF_CUSTOM_VIDEO", workflowVersionId: "WV_CUSTOM_VIDEO", recipeId: "R_CUSTOM_VIDEO", recipeVersion: "1.0.0", name: "Custom Text Video", category: "video", mode: "text_to_video", fields: [{ key: "prompt", type: "textarea", label: "提示词", required: true, default: "A test video" }], outputTypes: ["video"] };
beforeEach(() => { vi.resetAllMocks(); mocks.getProjectWorkflowConfig.mockResolvedValue({ projectId: "P_VIDEO", videoModeOverrides: [] }); mocks.assetLibraryPage.mockResolvedValue({ items: [], nextCursor: undefined }); mocks.listAssetVideoPrompts.mockResolvedValue([]); mocks.listPresets.mockResolvedValue([]); mocks.getPreferredPreset.mockResolvedValue(undefined); mocks.listModels.mockResolvedValue([]); mocks.listModelVersions.mockResolvedValue([]); });
afterEach(() => cleanup());
describe("generic Asset Video Production Queue submission", () => {
  it("does not submit or auto-start; redirects to formal Shot OCC bind path", async () => {
    const user = userEvent.setup(); const onOpenProductionQueue = vi.fn();
    render(<AssetVideoBatchWorkspace projectId="P_VIDEO" catalog={[recipe]} initialAssets={[]} comfyConnected taskEventsReady productionAdmission={{ busy: false }} onAdmissionChanged={vi.fn().mockResolvedValue(undefined)} onProductionBatchFocused={vi.fn()} onOpenTask={vi.fn()} onOpenProductionQueue={onOpenProductionQueue} onBackToAssets={vi.fn()} />);
    expect(await screen.findByText("请改用镜头正式输入槽")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "加入队列并开始" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "打开生产队列" }));
    expect(onOpenProductionQueue).toHaveBeenCalledWith();
    expect(mocks.submitGeneration).not.toHaveBeenCalled();
    expect(mocks.createProductionQueue).not.toHaveBeenCalled();
    expect(mocks.startProductionQueue).not.toHaveBeenCalled();
  });
});
