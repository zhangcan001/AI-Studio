import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ShotView } from "../../types/shot";
import { ProjectStructureTree, limitVisibleShotIds } from "./ProjectStructureTree";

const shot = (id: string, ordinal: number): ShotView => ({
  id,
  projectId: "project-1",
  ordinal,
  name: `镜头 ${id}`,
  promptText: "",
  createdAt: "",
  updatedAt: "",
  status: "DRAFT",
  imageStatus: "DRAFT",
  videoStatus: "DRAFT",
  stageConfigs: [],
  referenceAssets: [],
  generationLinks: [],
});

const tree: ProductionStructureTree = {
  projectId: "project-1",
  unassignedShotIds: ["shot-unassigned"],
  series: [{
    id: "series-1",
    projectId: "project-1",
    ordinal: 0,
    name: "第一季",
    description: "",
    createdAt: "",
    updatedAt: "",
    episodes: [{
      id: "episode-1",
      seriesId: "series-1",
      ordinal: 0,
      name: "第一集",
      description: "",
      createdAt: "",
      updatedAt: "",
      scenes: [
        {
          id: "scene-1",
          episodeId: "episode-1",
          ordinal: 0,
          name: "入口",
          description: "",
          shotIds: ["shot-1", "shot-2", "shot-3"],
          createdAt: "",
          updatedAt: "",
        },
        {
          id: "scene-2",
          episodeId: "episode-1",
          ordinal: 1,
          name: "审判",
          description: "",
          shotIds: ["shot-4"],
          createdAt: "",
          updatedAt: "",
        },
      ],
    }],
  }],
};

const shots = [shot("shot-1", 0), shot("shot-2", 1), shot("shot-3", 2), shot("shot-4", 3), shot("shot-unassigned", 4)];

function renderTree(selection?: Parameters<typeof ProjectStructureTree>[0]["selectedSelection"], maxVisibleShots?: number) {
  return renderToStaticMarkup(
    <ProjectStructureTree
      project={{ id: "project-1", name: "演示项目" }}
      tree={tree}
      shots={shots}
      selectedSelection={selection}
      onSelectSelection={() => undefined}
      onCreate={() => undefined}
      maxVisibleShots={maxVisibleShots}
      headerActions={<span data-header-slot="true">管理</span>}
    />,
  );
}

describe("ProjectStructureTree", () => {
  it("renders project, series, episode and scenes with series/episode expanded by default", () => {
    const html = renderTree({ type: "project", projectId: "project-1" });

    expect(html).toContain("演示项目");
    expect(html).toContain("第一季");
    expect(html).toContain("第一集");
    expect(html).toContain("入口");
    expect(html).toContain('aria-expanded="true"');
    expect(html).not.toContain("镜头 shot-1");
    expect(html).toContain('aria-haspopup="menu"');
    expect(html).toContain('data-header-slot="true"');
  });

  it("renders shots only for the current scene and exposes selection/current ARIA state", () => {
    const html = renderTree({ type: "scene", sceneId: "scene-1" });

    expect(html).toContain("镜头 shot-1");
    expect(html).toContain("镜头 shot-2");
    expect(html).not.toContain("镜头 shot-4");
    expect(html).toContain('data-node-type="scene"');
    expect(html).toContain('aria-selected="true"');
    expect(html).toContain('aria-current="true"');
    expect(html).toContain('aria-expanded="true"');
  });

  it("caps current-scene shot DOM and keeps a selected shot addressable", () => {
    expect(limitVisibleShotIds(["a", "b", "c", "d"], "d", 2)).toEqual(["a", "d"]);
    const html = renderTree({ type: "shot", shotId: "shot-3" }, 2);

    expect(html).toContain("镜头 shot-1");
    expect(html).toContain("镜头 shot-3");
    expect(html).not.toContain("镜头 shot-2");
    expect(html).toContain("还有 1 个镜头未展开");
  });

  it("keeps an unassigned selected shot visible without expanding all unassigned shots", () => {
    const html = renderTree({ type: "shot", shotId: "shot-unassigned" });

    expect(html).toContain("镜头 shot-unassigned");
    expect(html).not.toContain("镜头 shot-1");
    expect(html).toContain("未归档镜头");
  });
});
