// @vitest-environment jsdom

import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  ConsistencyBindingPack,
  ConsistencyProfileBindingInput,
  ConsistencyReferenceSetBindingInput,
} from "../../types/consistencyBindings";
import { ConsistencyBindingEditor } from "./ConsistencyBindingEditor";

const profiles = [
  { id: "profile-character", projectId: "project-1", profileType: "CHARACTER" as const, name: "赤羽" },
  { id: "profile-scene", projectId: "project-1", profileType: "SCENE" as const, name: "雨巷" },
  { id: "profile-prop", projectId: "project-1", profileType: "PROP" as const, name: "长刀" },
  { id: "profile-style", projectId: "project-1", profileType: "STYLE" as const, name: "电影感" },
];

const referenceSets = [
  { id: "set-character", projectId: "project-1", purpose: "CHARACTER" as const, name: "赤羽正面" },
  { id: "set-scene", projectId: "project-1", purpose: "SCENE" as const, name: "雨巷氛围" },
  { id: "set-shot", projectId: "project-1", purpose: "SHOT" as const, name: "本镜头参考" },
];

const directProfileBindings: ConsistencyProfileBindingInput[] = [{
  id: "binding-character",
  role: "CHARACTER",
  profileType: "CHARACTER",
  profileId: "profile-character",
  costumeVariantId: null,
  ordinal: 0,
  inheritanceMode: "EXPLICIT",
}];

const directReferenceSetBindings: ConsistencyReferenceSetBindingInput[] = [];

const pack: ConsistencyBindingPack = {
  scope: { scopeType: "SCENE", scopeId: "scene-1", scopeName: "雨巷" },
  ancestors: [],
  directProfileBindings,
  directReferenceSetBindings,
};

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("ConsistencyBindingEditor", () => {
  it("edits profile, costume, reference set, EXPLICIT/REPLACE/REMOVE and sends normalized wire DTOs", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn().mockResolvedValue(pack);

    render(
      <ConsistencyBindingEditor
        projectId="project-1"
        scopeType="SCENE"
        scopeId="scene-1"
        directProfileBindings={directProfileBindings}
        directReferenceSetBindings={directReferenceSetBindings}
        profiles={profiles}
        referenceSets={referenceSets}
        costumesByCharacter={{ "profile-character": [{ id: "costume-red", characterProfileId: "profile-character", name: "红衣" }] }}
        onSave={onSave}
      />,
    );

    const currentCharacter = screen.getByRole("article", { name: "角色档案绑定" });
    await user.selectOptions(within(currentCharacter).getByRole("combobox", { name: "角色服装" }), "costume-red");
    await user.selectOptions(within(currentCharacter).getByRole("combobox", { name: "绑定动作" }), "REPLACE");

    await user.click(screen.getByRole("button", { name: "添加档案绑定" }));
    const profileRows = screen.getAllByRole("article", { name: /档案绑定$/ });
    const newProfile = profileRows[profileRows.length - 1];
    await user.selectOptions(within(newProfile).getByRole("combobox", { name: "角色绑定角色" }), "PROP");
    await user.selectOptions(within(newProfile).getByRole("combobox", { name: "道具档案" }), "profile-prop");
    await user.selectOptions(within(newProfile).getByRole("combobox", { name: "绑定动作" }), "REMOVE");

    await user.click(screen.getByRole("button", { name: "添加参考集绑定" }));
    const referenceRow = screen.getByRole("article", { name: "角色参考集绑定" });
    await user.selectOptions(within(referenceRow).getByRole("combobox", { name: "角色参考集" }), "set-character");
    await user.click(within(referenceRow).getByRole("checkbox", { name: "生产必需" }));
    await user.selectOptions(within(referenceRow).getByRole("combobox", { name: "绑定动作" }), "EXPLICIT");

    expect(screen.queryByRole("option", { name: "继承" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "保存一致性配置" }));

    await vi.waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave.mock.calls[0][0]).toEqual({
      projectId: "project-1",
      scopeType: "SCENE",
      scopeId: "scene-1",
      profileBindings: [
        { ...directProfileBindings[0], costumeVariantId: "costume-red", inheritanceMode: "REPLACE", ordinal: 0 },
        { role: "PROP", profileType: "PROP", profileId: "profile-prop", costumeVariantId: null, inheritanceMode: "REMOVE", ordinal: 0 },
      ],
      referenceSetBindings: [{ role: "CHARACTER", referenceSetId: "set-character", ordinal: 0, required: true, inheritanceMode: "EXPLICIT" }],
    });
  });

  it("keeps inherited bindings read-only and exposes the two Asset shortcuts", async () => {
    const user = userEvent.setup();
    const onOpenAssets = vi.fn();
    render(
      <ConsistencyBindingEditor
        projectId="project-1"
        scopeType="PROJECT"
        scopeId="project-1"
        directProfileBindings={[]}
        directReferenceSetBindings={[]}
        inheritedProfileBindings={[{ ...directProfileBindings[0], inheritanceMode: "INHERITED" }]}
        profiles={[]}
        referenceSets={[]}
        onSave={vi.fn().mockResolvedValue(undefined)}
        onOpenAssets={onOpenAssets}
      />,
    );

    expect(screen.getByRole("region", { name: "上级继承配置" })).toBeTruthy();
    expect(screen.getByText("继承")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "前往资产库创建" }));
    await user.click(screen.getByRole("button", { name: "前往资产库管理参考集" }));
    expect(onOpenAssets.mock.calls).toEqual([["profiles"], ["referenceSets"]]);
  });
});
