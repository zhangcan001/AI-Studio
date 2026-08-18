import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ProductionStructureTree } from "../../types/productionStructure";
import type { ShotView } from "../../types/shot";
import { ProductionStructurePanel } from "./ProductionStructurePanel";

const tree: ProductionStructureTree = {
  projectId: "project-1",
  unassignedShotIds: ["shot-2"],
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
      scenes: [{
        id: "scene-1",
        episodeId: "episode-1",
        ordinal: 0,
        name: "入口",
        description: "",
        shotIds: ["shot-1"],
        createdAt: "",
        updatedAt: "",
      }],
    }],
  }],
};

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

describe("ProductionStructurePanel", () => {
  it("renders the existing project structure and bounded shot picker", () => {
    const html = renderToStaticMarkup(
      <ProductionStructurePanel
        projectId="project-1"
        tree={tree}
        shots={[shot("shot-1", 0), shot("shot-2", 1)]}
        selectedShotId="shot-1"
        onSelectShot={() => undefined}
        onChanged={() => undefined}
      />,
    );

    expect(html).toContain("内容结构");
    expect(html).toContain("第一季");
    expect(html).toContain("第一集");
    expect(html).toContain("入口");
    expect(html).toContain("取消所属场景");
    expect(html).toContain("镜头 shot-2");
  });
});
