// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import { WorkflowProductionProfiles } from "./WorkflowProductionProfiles";
import type { WorkflowProductionProfile } from "./workflowCenterModel";

const recipe: RecipeViewModel = {
  workflowId: "workflow-1",
  workflowVersionId: "version-1",
  recipeId: "recipe-1",
  name: "MiniMax H3",
  category: "video",
  mode: "text_to_video",
  fields: [],
  outputTypes: ["video"],
};

const profile: WorkflowProductionProfile = {
  key: "FL2VA_TEXT_TO_VIDEO",
  label: "文生视频",
  purpose: "项目视频生产路径",
  recipe,
  status: "READY",
  sourceLabel: "项目默认",
  configured: true,
  staleConfiguredBinding: false,
  usingFallback: false,
  runtimeProfileCount: 2,
  technicalIdentity: { workflowVersionId: "version-1", recipeId: "recipe-1" },
};

afterEach(() => cleanup());

describe("WorkflowProductionProfiles", () => {
  it("keeps business information primary and technical identity behind details", () => {
    render(
      <WorkflowProductionProfiles
        profiles={[profile]}
        onChangeWorkflow={vi.fn()}
        onManageParameters={vi.fn()}
      />,
    );

    expect(screen.getByRole("heading", { name: "文生视频" })).toBeTruthy();
    expect(screen.getByText("工作流：MiniMax H3")).toBeTruthy();
    expect(screen.getByText("可生产")).toBeTruthy();
    expect(screen.getByText("2 个")).toBeTruthy();
    expect(screen.getByText("查看技术详情").closest("details")?.open).toBe(false);
  });
});
