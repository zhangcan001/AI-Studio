import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it, vi } from "vitest";
import type { RecipeViewModel } from "../../types/generation";
import type { TaskView } from "../../types/task";
import { CreationResultPanel } from "./CreationResultPanel";
import { GenerationActionBar } from "./GenerationActionBar";
import { StudioModeTabs } from "./StudioModeTabs";
import { TaskProgressCard } from "./TaskProgressCard";
import { WorkflowLauncher } from "./WorkflowLauncher";

const catalog: RecipeViewModel[] = [
  {
    workflowId: "wfl_kera2_t2i_local_v2",
    workflowVersionId: "wfv_kera2",
    recipeId: "rcp_kera2",
    name: "Krea2 T2I Local",
    category: "image",
    mode: "text_to_image",
    fields: [],
  },
  {
    workflowId: "wfl_minimax_h3_reference_video",
    workflowVersionId: "wfv_h3",
    recipeId: "rcp_h3",
    name: "MiniMax H3 Reference Video",
    category: "video",
    mode: "reference_to_video",
    fields: [],
  },
];

describe("studio product UX contracts", () => {
  it("renders one selectable card per supported workflow and marks the current card", () => {
    const html = renderToStaticMarkup(
      <WorkflowLauncher catalog={catalog} selectedWorkflow={catalog[1]} onSelect={vi.fn()} />,
    );

    expect((html.match(/class="workflow-launcher-card/g) ?? []).length).toBe(2);
    expect(html).toContain("Kera2 文生图");
    expect(html).toContain("MiniMax H3 参考图生视频");
    expect(html).toContain('aria-pressed="true"');
  });

  it("keeps the two creation modes as presentation tabs", () => {
    const html = renderToStaticMarkup(<StudioModeTabs mode="batch" onChange={vi.fn()} />);

    expect(html).toContain("单次创作");
    expect(html).toContain("批量生产");
    expect(html).toContain('aria-selected="true"');
  });

  it("keeps the main generate action visible while exposing a single blocked reason", () => {
    const html = renderToStaticMarkup(
      <GenerationActionBar
        creating={false}
        canGenerate={false}
        canAddToBatch={false}
        blockedReason="请先选择所需素材。"
        batchCount={0}
        onGenerate={vi.fn()}
        onAddToBatch={vi.fn()}
      />,
    );

    expect(html).toContain("开始生成");
    expect(html).toContain("请先选择所需素材。");
    expect((html.match(/请先选择所需素材。/g) ?? []).length).toBe(1);
  });

  it("keeps the empty result state to one right-column panel and makes success compact", () => {
    const emptyHtml = renderToStaticMarkup(
      <CreationResultPanel projectId="prj_default" cancelling={false} onCancel={vi.fn()} />,
    );
    const task: TaskView = {
      id: "tsk_smoke",
      projectId: "prj_default",
      status: "SUCCEEDED",
      progress: { mode: "indeterminate" },
      createdAt: "2026-08-09T00:00:00.000Z",
      finishedAt: "2026-08-09T00:00:42.000Z",
      outputAssetIds: [],
    };
    const successHtml = renderToStaticMarkup(<TaskProgressCard task={task} />);

    expect((emptyHtml.match(/class=\"output-card/g) ?? []).length).toBe(1);
    expect(emptyHtml).not.toContain("class=\"task-card");
    expect(successHtml).toContain("生成完成");
    expect(successHtml).toContain("用时");
    expect(successHtml).toContain("compact-task-card");
  });
});
