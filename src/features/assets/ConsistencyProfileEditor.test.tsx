import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type {
  ConsistencyProfileView,
  CostumeVariantView,
  ReferenceSetSummary,
} from "../../types/consistency";
import { ConsistencyProfileEditor } from "./ConsistencyProfileEditor";

const profile: ConsistencyProfileView = {
  id: "profile-1",
  projectId: "project-1",
  profileType: "CHARACTER",
  name: "林间旅人",
  description: "主角",
  canonicalPrompt: "短发、红围巾",
  negativePrompt: null,
  defaultReferenceSetId: "set-1",
  defaultStyleProfileId: "style-1",
  metadataJson: "{}",
  activeRevisionId: "revision-1",
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

const costume: CostumeVariantView = {
  id: "costume-1",
  characterProfileId: "profile-1",
  name: "雨衣",
  promptFragment: "黄色雨衣",
  referenceSetId: "set-costume",
  isDefault: true,
  ordinal: 0,
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

const referenceSet: ReferenceSetSummary = {
  id: "set-1",
  projectId: "project-1",
  name: "主角参考集",
  purpose: "CHARACTER",
  description: "",
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

describe("ConsistencyProfileEditor", () => {
  it("supports create and edit forms for the profile types", () => {
    const createHtml = renderToStaticMarkup(
      <ConsistencyProfileEditor
        profileType="SCENE"
        onSave={async () => undefined}
      />,
    );
    const editHtml = renderToStaticMarkup(
      <ConsistencyProfileEditor
        profileType="CHARACTER"
        profile={profile}
        costumes={[costume]}
        referenceSets={[referenceSet]}
        onSave={async () => undefined}
        onCostumeCreate={async () => undefined}
        onCostumeUpdate={async () => undefined}
        onCostumeDelete={async () => undefined}
      />,
    );

    expect(createHtml).toContain("新建场景档案");
    expect(createHtml).toContain("环境提示词");
    expect(editHtml).toContain("编辑角色档案");
    expect(editHtml).toContain("林间旅人");
    expect(editHtml).toContain("服装变体");
    expect(editHtml).toContain("雨衣");
    expect(editHtml).toContain("新增服装变体");
  });
});
