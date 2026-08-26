import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { ReferenceSetDetailView, ReferenceSetItemView } from "../../types/consistency";
import { MAX_REFERENCE_SET_ITEMS } from "../../types/consistency";
import { normalizeReferenceSetItems, ReferenceSetEditor } from "./ReferenceSetEditor";

const item = (index: number): ReferenceSetItemView => ({
  assetId: `asset-${index}`,
  ordinal: 99,
  role: index === 0 ? " FACE " : null,
  isPrimary: index === 1,
  assetName: `参考图 ${index + 1}`,
  thumbnailAvailable: false,
  width: 1200,
  height: 800,
});

const detail: ReferenceSetDetailView = {
  referenceSet: {
    id: "set-1",
    projectId: "project-1",
    name: "主角参考集",
    purpose: "CHARACTER",
    description: "",
    ownerProfileType: "CHARACTER",
    ownerProfileId: "profile-1",
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  },
  items: Array.from({ length: MAX_REFERENCE_SET_ITEMS }, (_, index) => item(index)),
};

describe("ReferenceSetEditor", () => {
  it("normalizes the saved order and caps items at twenty", () => {
    const normalized = normalizeReferenceSetItems(Array.from({ length: MAX_REFERENCE_SET_ITEMS + 2 }, (_, index) => item(index)));

    expect(normalized).toHaveLength(MAX_REFERENCE_SET_ITEMS);
    expect(normalized.map((entry) => entry.ordinal)).toEqual(Array.from({ length: MAX_REFERENCE_SET_ITEMS }, (_, index) => index));
    expect(normalized[0].role).toBe("FACE");
    expect(normalized[1].role).toBeNull();
    expect(normalized[1].isPrimary).toBe(true);
  });

  it("renders reorder, primary, role shortcuts, and the max twenty state", () => {
    const html = renderToStaticMarkup(
      <ReferenceSetEditor
        projectId="project-1"
        detail={detail}
        ownerProfiles={[]}
        onSave={async () => undefined}
      />,
    );

    expect(html).toContain("有序参考图（20/20）");
    expect(html).toContain("上移");
    expect(html).toContain("下移");
    expect(html).toContain("设为主图");
    expect(html).toContain("FACE");
    expect(html).toContain("自定义 role");
  });
});
