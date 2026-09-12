// @vitest-environment jsdom

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { WorkspaceSelection } from "../../../types/workspaceSelection";
import { useShotWorkspaceSelection } from "./useShotWorkspaceSelection";

afterEach(() => {
  cleanup();
});

function Harness({ projectId, initialSelectedShotId, onShotSelected }: {
  projectId: string;
  initialSelectedShotId?: string;
  onShotSelected: (shotId?: string) => void;
}) {
  const selection = useShotWorkspaceSelection({ projectId, initialSelectedShotId, onShotSelected });
  const select = (next: WorkspaceSelection) => selection.selectWorkspaceSelection(next);
  return (
    <div>
      <output data-testid="selected-shot">{selection.selectedShotId ?? "none"}</output>
      <output data-testid="selection-type">{selection.workspaceSelection.type}</output>
      <button type="button" onClick={() => select({ type: "shot", shotId: "shot-2" })}>select-shot-2</button>
      <button type="button" onClick={() => select({ type: "scene", sceneId: "scene-1" })}>select-scene</button>
      <button type="button" onClick={() => selection.reconcileSelectedShot(["shot-3"])}>reconcile-shot-3</button>
      <button type="button" onClick={() => selection.reconcileSelectedShot(["shot-1", "shot-2"])}>reconcile-available-shots</button>
      <button type="button" onClick={() => selection.reconcileSelectedShot([])}>reconcile-empty</button>
    </div>
  );
}

describe("useShotWorkspaceSelection", () => {
  it("preserves the deep-link target, notifies shot navigation, and reconciles removed shots", async () => {
    const user = userEvent.setup();
    const onShotSelected = vi.fn();
    render(<Harness projectId="project-1" initialSelectedShotId="shot-1" onShotSelected={onShotSelected} />);

    expect(screen.getByTestId("selected-shot").textContent).toBe("shot-1");
    expect(screen.getByTestId("selection-type").textContent).toBe("shot");

    await user.click(screen.getByRole("button", { name: "select-shot-2" }));
    expect(screen.getByTestId("selected-shot").textContent).toBe("shot-2");
    expect(onShotSelected).toHaveBeenLastCalledWith("shot-2");

    await user.click(screen.getByRole("button", { name: "reconcile-shot-3" }));
    expect(screen.getByTestId("selected-shot").textContent).toBe("shot-3");
    expect(onShotSelected).toHaveBeenLastCalledWith("shot-3");

    await user.click(screen.getByRole("button", { name: "reconcile-empty" }));
    expect(screen.getByTestId("selected-shot").textContent).toBe("none");
    expect(screen.getByTestId("selection-type").textContent).toBe("project");
    expect(onShotSelected).toHaveBeenLastCalledWith(undefined);
  });

  it("does not replace a non-shot structural selection while shot data reloads", async () => {
    const user = userEvent.setup();
    render(<Harness projectId="project-1" onShotSelected={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "select-scene" }));
    await user.click(screen.getByRole("button", { name: "reconcile-empty" }));
    expect(screen.getByTestId("selection-type").textContent).toBe("scene");
  });

  it("does not silently fall back when an explicit target is unavailable", async () => {
    const user = userEvent.setup();
    const onShotSelected = vi.fn();
    render(<Harness projectId="project-1" initialSelectedShotId="missing-shot" onShotSelected={onShotSelected} />);

    await user.click(screen.getByRole("button", { name: "reconcile-available-shots" }));
    expect(screen.getByTestId("selected-shot").textContent).toBe("none");
    expect(onShotSelected).not.toHaveBeenCalled();
  });
});
