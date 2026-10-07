// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { CreatePage } from "./CreatePage";
import { useStudioStore } from "../../stores/studioStore";
import type { CreationContext, CreationReadiness, GeneratorOption } from "../../product/types";
import type { ComfyStatus } from "../../types/comfy";

const api = vi.hoisted(() => ({ get: vi.fn(), generatorsList: vi.fn(), readinessGet: vi.fn(), generate: vi.fn(), runGet: vi.fn() }));
vi.mock("../../product/client", () => ({ productClient: { creation: { ...api, mediaUrl: () => "http://fixture.invalid/video" }, run: { get: api.runGet } } }));
const route = { kind: "create", projectId: "project", shotId: "shot", stage: "video" } as const;
const online: ComfyStatus & { runtimeGeneration: number } = { status: "CONNECTED", endpoint: "http://fixture.invalid", devices: [], runtimeGeneration: 1 };
const ready: CreationReadiness = { ready: true, issues: [], fieldErrors: [], actions: [] };
const blocked: CreationReadiness = { ...ready, ready: false };
const generator: GeneratorOption = { selectionRef: "exact-selection", name: "H3", version: "2.2.0", mode: "T2V", mediaKind: "video", availability: true, availabilityReason: null, recommended: true, fields: [{ type: "textarea", key: "prompt", label: "Prompt", required: true, default: "" }] };
const context: CreationContext = { projectId: "project", projectName: "Project", stage: "video", shots: [{ id: "shot", name: "Shot", ordinal: 0 }], selectedShot: { summary: { id: "shot", name: "Shot", ordinal: 0 }, prompt: "current prompt", selectionRef: null, values: {}, referenceAssetIds: [], selectedResultId: null, recentRun: null }, candidates: [], mediaInputs: [], promptChoices: [] };
const navigate = vi.fn();
const page = (runtime: ComfyStatus & { runtimeGeneration: number }) => <CreatePage route={route} navigate={navigate} {...{ runtime }} />;
function deferred() { let resolve!: (result: CreationReadiness) => void; const promise = new Promise<CreationReadiness>(yes => { resolve = yes; }); return { promise, resolve }; }
async function settle() { await act(async () => { await vi.advanceTimersByTimeAsync(600); }); }
async function start() { const view = render(page(online)); await act(async () => {}); await settle(); return view; }

beforeEach(() => {
  vi.useFakeTimers(); vi.resetAllMocks(); useStudioStore.getState().resetDraft();
  api.get.mockResolvedValue(context); api.generatorsList.mockResolvedValue([generator]); api.readinessGet.mockResolvedValue(ready);
  api.generate.mockResolvedValue({ accepted: true, runRef: { source: "queue-batch", id: "run" }, startOutcome: "STARTED", startIssue: null });
  api.runGet.mockResolvedValue({ ref: { source: "queue-batch", id: "run" }, status: "QUEUED", progress: { total: 1, succeeded: 0 }, availableActions: [] });
});
afterEach(() => { cleanup(); vi.useRealTimers(); });

describe("Create runtime readiness lifecycle", () => {
  it("invalidates cached READY immediately when authoritative health becomes offline", async () => {
    const view = await start(); expect(screen.getByText("可以生成")).toBeTruthy();
    view.rerender(page({ ...online, status: "OFFLINE", runtimeGeneration: 2 }));
    expect(screen.queryByText("可以生成")).toBeNull();
    expect(screen.getByText("运行环境暂时不可用")).toBeTruthy();
    expect(useStudioStore.getState().values.prompt).toEqual({ type: "string", value: "current prompt" });
  });
  it("automatically reruns readiness on reconnect without editing the draft", async () => {
    const view = await start(); view.rerender(page({ ...online, status: "OFFLINE", runtimeGeneration: 2 }));
    const calls = api.readinessGet.mock.calls.length;
    view.rerender(page({ ...online, runtimeGeneration: 2 }));
    expect(screen.queryByText("可以生成")).toBeNull(); await settle();
    expect(api.readinessGet).toHaveBeenCalledTimes(calls + 1);
    expect(screen.getByText("可以生成")).toBeTruthy();
  });
  it("ignores an old READY completion after disconnect", async () => {
    const pending = deferred(); api.readinessGet.mockReturnValueOnce(pending.promise);
    const view = await start(); view.rerender(page({ ...online, status: "OFFLINE", runtimeGeneration: 2 }));
    await act(async () => { pending.resolve(ready); });
    expect(screen.queryByText("可以生成")).toBeNull(); expect(screen.getByText("运行环境暂时不可用")).toBeTruthy();
  });
  it("ignores old completion after reconnect has advanced the runtime generation", async () => {
    const old = deferred(), current = deferred(); api.readinessGet.mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise);
    const view = await start(); view.rerender(page({ ...online, status: "OFFLINE", runtimeGeneration: 2 })); view.rerender(page({ ...online, runtimeGeneration: 2 }));
    await settle(); await act(async () => { old.resolve(ready); });
    expect(screen.queryByText("可以生成")).toBeNull();
    await act(async () => { current.resolve(ready); }); expect(screen.getByText("可以生成")).toBeTruthy();
  });
  it("still calls fresh backend readiness for Generate while UI is READY", async () => {
    await start(); const calls = api.readinessGet.mock.calls.length;
    fireEvent.click(screen.getByRole("button", { name: "生成" })); await act(async () => {});
    expect(api.readinessGet).toHaveBeenCalledTimes(calls + 1);
    expect(api.readinessGet).toHaveBeenLastCalledWith(expect.objectContaining({ selectionRef: "exact-selection", submissionIdempotencyKey: expect.not.stringMatching(/^readiness-only$/) }));
    expect(api.generate).toHaveBeenCalledTimes(1);
  });
  it("does not invoke queue admission when offline fresh readiness blocks Generate", async () => {
    const view = await start(); view.rerender(page({ ...online, status: "OFFLINE", runtimeGeneration: 2 })); api.readinessGet.mockResolvedValue(blocked);
    fireEvent.click(screen.getByRole("button", { name: "生成" })); await act(async () => {});
    expect(api.readinessGet).toHaveBeenLastCalledWith(expect.objectContaining({ projectId: "project", shotId: "shot" }));
    expect(api.generate).not.toHaveBeenCalled(); // The only creation authority creates task/batch/item together.
    expect(screen.queryByText("可以生成")).toBeNull();
  });
  it("rejects the reconnect request if inputs changed while it was in flight", async () => {
    const view = await start(); view.rerender(page({ ...online, status: "OFFLINE", runtimeGeneration: 2 }));
    const old = deferred(), current = deferred(); api.readinessGet.mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise);
    view.rerender(page({ ...online, runtimeGeneration: 2 })); await settle();
    fireEvent.change(screen.getByLabelText("提示词"), { target: { value: "new prompt" } }); await settle();
    await act(async () => { old.resolve(ready); }); expect(screen.queryByText("可以生成")).toBeNull();
    expect(api.readinessGet).toHaveBeenLastCalledWith(expect.objectContaining({ values: expect.objectContaining({ prompt: { type: "string", value: "new prompt" } }) }));
    await act(async () => { current.resolve(blocked); }); expect(screen.getByText("请检查输入或运行环境")).toBeTruthy();
  });
  it.each([{ runtimeGeneration: 2 }, { endpoint: "http://new-runtime.invalid" }])("invalidates a positive result when runtime identity changes: %s", async change => {
    const view = await start(); view.rerender(page({ ...online, ...change }));
    expect(screen.queryByText("可以生成")).toBeNull(); await settle(); expect(screen.getByText("可以生成")).toBeTruthy();
  });
  it("does not admit an old submit-time READY after disconnect during the fresh check", async () => {
    const view = await start(); const pending = deferred(); api.readinessGet.mockReturnValueOnce(pending.promise);
    fireEvent.click(screen.getByRole("button", { name: "生成" })); view.rerender(page({ ...online, status: "OFFLINE", runtimeGeneration: 2 }));
    await act(async () => { pending.resolve(ready); });
    expect(api.generate).not.toHaveBeenCalled(); expect(screen.queryByText("可以生成")).toBeNull();
  });
});
