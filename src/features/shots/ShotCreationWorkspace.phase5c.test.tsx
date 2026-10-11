// @vitest-environment jsdom
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { RunResult } from "../../product/types";
import type { AssetView } from "../../types/asset";
import type { ShotView } from "../../types/shot";
import { ShotCreationWorkspace, canConfirmShotCandidate } from "./ShotCreationWorkspace";
const api = vi.hoisted(() => ({ results: vi.fn(), image: vi.fn(), thumb: vi.fn(), confirm: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { run: { resultsGet: api.results } } }));
vi.mock("../../services/tauriClient", async original => ({ ...await original<typeof import("../../services/tauriClient")>(), readAssetImage: api.image, readAssetThumbnail: api.thumb, getAssetMediaUrl: (p: string, id: string) => `https://owned.invalid/${p}/${id}` }));
const asset: AssetView = { id: "video", assetType: "video", category: "generated_video", name: "video", originalName: "video.mp4", mimeType: "video/mp4", fileSize: 100, createdAt: "", sourceTaskId: "task", thumbnailAvailable: false, isFavorite: false, tags: [] };
const shot: ShotView = { id: "shot", projectId: "p", name: "shot", ordinal: 0, promptText: "move", status: "DRAFT", imageStatus: "DRAFT", videoStatus: "VIDEO_REVIEW", stageConfigs: [], referenceAssets: [], generationLinks: [], createdAt: "", updatedAt: "" };
const result = (reviewState: string): RunResult => ({ assetId: "video", name: "video", mediaKind: "video", assetExists: true, availability: "available", reviewState, reviewRevision: 0, selectedShotIds: [], thumbnailBytes: null });
function Host({ projectId = "p", currentShot = shot, recipeId = "ra", thumbnail = false }: { projectId?: string; currentShot?: ShotView; recipeId?: string; thumbnail?: boolean }) {
  const [selected, setSelected] = useState<string>();
  return <ShotCreationWorkspace projectId={projectId} shot={currentShot} stage="video" onStageChange={vi.fn()} onGenerate={vi.fn()} currentRecipe={{ workflowId: "w", workflowVersionId: "v", recipeId, name: recipeId, category: "video", mode: "text_to_video", fields: [] }} candidates={[{ asset: { ...asset, thumbnailAvailable: thumbnail }, taskId: "task", fromLinkedTask: true }]} selectedAssetId={selected} onCandidateSelect={vi.fn()} onCandidateConfirm={(id, linked) => { api.confirm(id, linked); setSelected(id); }} />;
}
beforeEach(() => { vi.clearAllMocks(); api.results.mockResolvedValue([result("PENDING")]); api.image.mockResolvedValue(new Uint8Array([1])); api.thumb.mockResolvedValue(new Uint8Array([1])); });
afterEach(() => { cleanup(); vi.unstubAllGlobals(); });
it("keeps preview, explicit Shot selection and persisted human review independent", async () => {
  const user = userEvent.setup(); render(<Host />);
  const facts = await screen.findByRole("region", { name: "候选选用与人工审核" });
  await within(facts).findByText("待审核"); expect(within(facts).getByText("镜头未选用")).toBeTruthy();
  await user.click(screen.getByRole("button", { name: /待确认/ })); expect(api.confirm).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "确认当前候选" })); expect(api.confirm).toHaveBeenCalledWith("video", true);
  expect(within(facts).getByText("镜头已选用")).toBeTruthy(); expect(within(facts).getByText("待审核")).toBeTruthy();
});
it("does not infer selection from approval, or remove selection because the review was rejected", async () => {
  expect(canConfirmShotCandidate({ asset, status: "reviewed" }, "other")).toBe(true);
  api.results.mockResolvedValue([result("REJECTED")]); render(<Host />);
  await screen.findByText("已驳回"); await userEvent.setup().click(screen.getByRole("button", { name: "确认当前候选" }));
  expect(screen.getByText("镜头已选用")).toBeTruthy(); expect(screen.getByText("已驳回")).toBeTruthy();
});
it("fences delayed result facts when project/Shot ownership changes", async () => {
  let finish!: (r: RunResult[]) => void; api.results.mockReturnValueOnce(new Promise(resolve => { finish = resolve; })).mockResolvedValue([result("REJECTED")]);
  const view = render(<Host />); await waitFor(() => expect(api.results).toHaveBeenCalledTimes(1));
  view.rerender(<Host projectId="new" currentShot={{ ...shot, id: "new-shot", projectId: "new" }} />);
  await screen.findByText("已驳回"); await act(async () => finish([result("APPROVED")]));
  expect(screen.queryByText("已通过")).toBeNull(); expect(api.results).toHaveBeenCalledWith("new", { source: "task", id: "task" });
});
it("does not substitute a different artifact returned for the task", async () => {
  api.results.mockResolvedValue([{ ...result("APPROVED"), assetId: "foreign" }]); render(<Host />);
  await screen.findByText("未找到当前候选的正式产物"); expect(screen.queryByText("已通过")).toBeNull();
});
it("fences review facts after a same-Shot Recipe switch", async () => {
  let finish!: (r: RunResult[]) => void; api.results.mockReturnValueOnce(new Promise(resolve => { finish = resolve; })).mockResolvedValue([result("REJECTED")]);
  const view = render(<Host />); await waitFor(() => expect(api.results).toHaveBeenCalledTimes(1));
  view.rerender(<Host recipeId="rb" />); await screen.findByText("已驳回");
  await act(async () => finish([result("APPROVED")])); expect(screen.queryByText("已通过")).toBeNull();
});
it("releases owned thumbnail URLs and replaces the Native player across project scope", async () => {
  const NativeURL = URL, created: string[] = [], revoke = vi.fn();
  vi.stubGlobal("URL", class extends NativeURL { static createObjectURL() { const url = `blob:owned-${created.length}`; created.push(url); return url; } static revokeObjectURL = revoke; });
  const view = render(<Host thumbnail />); await waitFor(() => expect(created.length).toBeGreaterThan(0));
  const previous = document.querySelector("video.shot-media-preview-main");
  view.rerender(<Host thumbnail projectId="new" currentShot={{ ...shot, projectId: "new", id: "new-shot" }} />);
  expect(document.querySelector("video.shot-media-preview-main")).not.toBe(previous);
  for (const media of document.querySelectorAll("video")) expect(media.getAttribute("src")).toContain("/new/");
  view.unmount(); expect(new Set(revoke.mock.calls.map(([url]) => url))).toEqual(new Set(created));
});
