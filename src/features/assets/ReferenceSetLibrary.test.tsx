import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ReferenceSetUsageSummary, UsageRelation } from "../../types/consistency";
import { ReferenceSetLibrary, usageRelations } from "./ReferenceSetLibrary";

const relation = (name: string, blocking = false): UsageRelation => ({
  entityType: "profile",
  entityId: name,
  displayName: name,
  relationType: "default_reference_set",
  blocking,
  detail: name,
});

describe("ReferenceSetLibrary", () => {
  it("renders purpose filters and explicit legacy-anchor conversion", () => {
    const html = renderToStaticMarkup(<ReferenceSetLibrary projectId="project-1" />);

    expect(html).toContain("参考集");
    expect(html).toContain("全部");
    expect(html).toContain("角色");
    expect(html).toContain("服装");
    expect(html).toContain("镜头");
    expect(html).toContain("从旧参考锚点创建");
    expect(html).toContain("新建参考集");
  });

  it("collects backend usage buckets, including owner and blocking relations", () => {
    const usage: ReferenceSetUsageSummary = {
      referenceSetId: "set-1",
      total: 2,
      blockingCount: 1,
      profileDefaults: [relation("角色档案", true)],
      costumes: [],
      shotBindings: [],
      scopeBindings: [],
      owner: relation("所有者"),
      itemCount: 2,
      items: [],
    };

    expect(usageRelations(usage).map((item) => item.displayName)).toEqual(["角色档案", "所有者"]);
    expect(usageRelations(usage)[0].blocking).toBe(true);
  });
});
