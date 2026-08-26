import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ProfileUsageSummary, UsageRelation } from "../../types/consistency";
import { ConsistencyProfileLibrary, usageBlockers } from "./ConsistencyProfileLibrary";

const blockingRelation: UsageRelation = {
  entityType: "referenceSet",
  entityId: "set-1",
  displayName: "主角参考集",
  relationType: "profile_default",
  blocking: true,
  detail: "档案默认参考集",
};

const emptyUsage = (overrides: Partial<ProfileUsageSummary> = {}): ProfileUsageSummary => ({
  profileId: "profile-1",
  profileType: "CHARACTER",
  total: 1,
  blockingCount: 1,
  shotBindings: [],
  scopeBindings: [],
  referenceSets: [],
  defaultStyleProfiles: [],
  costumeVariants: [],
  relatedProfiles: [],
  items: [],
  ...overrides,
});

describe("ConsistencyProfileLibrary", () => {
  it("renders profile type filters, name filter, and create entry point", () => {
    const html = renderToStaticMarkup(<ConsistencyProfileLibrary projectId="project-1" />);

    expect(html).toContain("档案库");
    expect(html).toContain("角色");
    expect(html).toContain("场景");
    expect(html).toContain("道具");
    expect(html).toContain("风格");
    expect(html).toContain("按名称筛选");
    expect(html).toContain("新建角色档案");
  });

  it("keeps blocking profile relations available to the delete guard", () => {
    const usage = emptyUsage({ items: [blockingRelation] });

    expect(usageBlockers(usage)).toEqual([blockingRelation]);
    expect(usageBlockers(emptyUsage({ blockingCount: 0 }))).toEqual([]);
  });
});
