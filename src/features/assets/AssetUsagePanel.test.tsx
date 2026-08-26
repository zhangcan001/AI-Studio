import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { AssetUsageSummary, UsageRelation } from "../../types/consistency";
import { AssetUsagePanel, usageBucketItems } from "./AssetUsagePanel";

const usageRelation = (relationType: string, name: string): UsageRelation => ({
  entityType: "shot",
  entityId: name,
  displayName: name,
  relationType,
  blocking: false,
  detail: name,
});

const summary: AssetUsageSummary = {
  assetId: "asset-1",
  total: 3,
  blockingCount: 1,
  referenceSets: [usageRelation("reference_set_item", "主角参考集")],
  profiles: [usageRelation("profile_default", "主角档案")],
  shots: [usageRelation("selected_keyframe", "镜头 01")],
  legacyReferences: [],
  productionHistory: [usageRelation("production_history", "历史任务")],
  items: [],
};

describe("AssetUsagePanel", () => {
  it("keeps usage buckets and selected keyframe fallback visible", () => {
    expect(usageBucketItems(summary, "referenceSets")[0].displayName).toBe("主角参考集");
    expect(usageBucketItems(summary, "profiles")[0].displayName).toBe("主角档案");
    expect(usageBucketItems(summary, "selectedKeyframes")[0].displayName).toBe("镜头 01");
    expect(usageBucketItems(summary, "productionHistory")[0].displayName).toBe("历史任务");
  });

  it("renders a lazy, selected-asset usage section", () => {
    const html = renderToStaticMarkup(<AssetUsagePanel projectId="project-1" assetId="asset-1" assetName="主角参考图" />);

    expect(html).toContain("Asset Usage");
    expect(html).toContain("使用情况");
    expect(html).toContain("正在加载使用情况");
  });
});
