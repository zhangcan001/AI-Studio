// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type {
  ConsistencyBindingPack,
  ConsistencyContextPreview,
} from "../../types/consistencyBindings";
import { ScopeConsistencyWorkspace } from "./ScopeConsistencyWorkspace";

const scope = { scopeType: "SCENE" as const, scopeId: "scene-1", scopeName: "雨巷" };
const profile = { id: "profile-character", projectId: "project-1", profileType: "CHARACTER" as const, name: "赤羽" };
const referenceSet = { id: "set-scene", projectId: "project-1", purpose: "SCENE" as const, name: "雨巷参考" };

const pack: ConsistencyBindingPack = {
  scope,
  ancestors: [{
    scopeType: "PROJECT",
    scopeId: "project-1",
    scopeName: "项目一",
    profileBindings: [{ role: "CHARACTER", profileType: "CHARACTER", profileId: profile.id, ordinal: 0, inheritanceMode: "EXPLICIT" }],
    referenceSetBindings: [],
  }],
  directProfileBindings: [],
  directReferenceSetBindings: [],
};

const context: ConsistencyContextPreview = {
  contextHash: "context-hash-123456",
  partial: true,
  diagnostics: [{ severity: "ERROR", code: "CONTEXT_PROFILE_NOT_FOUND", message: "角色档案不存在" }],
  sourceTrace: [
    { scope: "PROJECT", scopeId: "project-1", scopeName: "项目一" },
    { scope: "SCENE", scopeId: "scene-1", scopeName: "雨巷" },
  ],
  profiles: [{ role: "CHARACTER", profileType: "CHARACTER", profileId: profile.id, name: profile.name, ordinal: 0, source: { scope: "PROJECT", scopeId: "project-1", scopeName: "项目一" } }],
  referenceSets: [{ role: "SCENE", referenceSetId: referenceSet.id, name: referenceSet.name, ordinal: 0, required: true, assetCount: 2, source: { scope: "SCENE", scopeId: "scene-1", scopeName: "雨巷" }, previewAssets: [{ assetId: "asset-1", name: "雨巷远景" }] }],
  promptText: "赤羽站在雨巷",
  negativePrompt: "低清晰度",
  readinessStatus: "INCOMPLETE",
  legacy: { usesLegacyShotReferences: false },
};

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

function renderWorkspace(overrides: Partial<React.ComponentProps<typeof ScopeConsistencyWorkspace>> = {}) {
  return render(
    <ScopeConsistencyWorkspace
      projectId="project-1"
      scope={scope}
      scopeOptions={[{ scopeType: "PROJECT", scopeId: "project-1", scopeName: "项目一" }, scope, { scopeType: "SHOT", scopeId: "shot-1", scopeName: "镜头一" }]}
      bindingPack={pack}
      onSaveBindingPack={vi.fn().mockResolvedValue(pack)}
      profiles={[profile]}
      referenceSets={[referenceSet]}
      context={context}
      {...overrides}
    />,
  );
}

describe("ScopeConsistencyWorkspace", () => {
  it("shows ancestor/direct/resolved sections, contextHash, prompt, partial diagnostics, and protects dirty scope navigation", async () => {
    const user = userEvent.setup();
    const onScopeChange = vi.fn();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    renderWorkspace({ onScopeChange });

    expect(screen.getByRole("heading", { level: 2, name: "场景一致性" })).toBeTruthy();
    expect(screen.getByRole("region", { name: "上级继承配置" })).toBeTruthy();
    expect(screen.getByText("context-hash-123456")).toBeTruthy();
    expect(screen.getByText("最终解析提示词")).toBeTruthy();
    expect(screen.getByText("赤羽站在雨巷")).toBeTruthy();
    expect(screen.getByText("低清晰度")).toBeTruthy();
    expect(screen.getByText("解析不完整；请查看下方 diagnostics，Readiness 仍以后端为准。")).toBeTruthy();
    expect(screen.getByText("CONTEXT_PROFILE_NOT_FOUND")).toBeTruthy();
    expect(screen.getByText("INCOMPLETE", { exact: false })).toBeTruthy();
    expect(screen.getByText("雨巷远景")).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "添加档案绑定" }));
    await user.click(screen.getByRole("button", { name: "项目 · 项目一" }));
    expect(confirm).toHaveBeenCalledWith("当前一致性配置尚未保存，确定切换结构范围吗？");
    expect(onScopeChange).not.toHaveBeenCalled();

    confirm.mockReturnValue(true);
    await user.click(screen.getByRole("button", { name: "项目 · 项目一" }));
    expect(onScopeChange).toHaveBeenCalledWith({ scopeType: "PROJECT", scopeId: "project-1", scopeName: "项目一" });
  });

  it("saves one binding pack, keeps backend-returned rows, and preserves the form after a failed save", async () => {
    const user = userEvent.setup();
    const savedPack: ConsistencyBindingPack = {
      ...pack,
      directProfileBindings: [{ role: "CHARACTER", profileType: "CHARACTER", profileId: profile.id, ordinal: 0, inheritanceMode: "EXPLICIT" }],
    };
    const onSave = vi.fn().mockResolvedValue(savedPack);
    renderWorkspace({ onSaveBindingPack: onSave });

    await user.click(screen.getByRole("button", { name: "添加档案绑定" }));
    await user.click(screen.getByRole("button", { name: "保存一致性配置" }));
    await vi.waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    expect(onSave.mock.calls[0][0]).toMatchObject({
      projectId: "project-1",
      scopeType: "SCENE",
      scopeId: "scene-1",
      profileBindings: [{ role: "CHARACTER", profileId: profile.id, ordinal: 0, inheritanceMode: "EXPLICIT" }],
      referenceSetBindings: [],
    });
    expect(screen.getAllByRole("status").some((element) => element.textContent?.includes("一致性配置已保存，并已重新读取后端真值。"))).toBe(true);

    cleanup();
    const failedSave = vi.fn().mockRejectedValue(new Error("Binding Pack 保存失败"));
    renderWorkspace({ onSaveBindingPack: failedSave });
    await user.click(screen.getByRole("button", { name: "添加档案绑定" }));
    await user.click(screen.getByRole("button", { name: "保存一致性配置" }));
    await vi.waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Binding Pack 保存失败"));
    expect(screen.getAllByRole("article", { name: /档案绑定$/ })).toHaveLength(1);
  });

  it("shows the legacy fallback without inventing a new reference pack", () => {
    renderWorkspace({
      context: {
        contextHash: null,
        partial: false,
        diagnostics: [],
        promptText: null,
        negativePrompt: null,
        legacy: { usesLegacyShotReferences: true, prompt: "旧版镜头参考提示词" },
      },
    });

    expect(screen.getByText("当前使用旧版镜头参考素材")).toBeTruthy();
    expect(screen.getByText("旧版镜头参考提示词")).toBeTruthy();
    expect(screen.getAllByText("—")).toHaveLength(3);
    expect(screen.queryByText("一致性参考集已接管本镜头参考输入")).toBeNull();
  });

  it("offers Asset shortcuts from an empty scope editor", async () => {
    const user = userEvent.setup();
    const onOpenAssets = vi.fn();
    renderWorkspace({ profiles: [], referenceSets: [], onOpenAssets });

    await user.click(screen.getByRole("button", { name: "前往资产库创建" }));
    await user.click(screen.getByRole("button", { name: "前往资产库管理参考集" }));
    expect(onOpenAssets.mock.calls).toEqual([["profiles"], ["referenceSets"]]);
  });
});
