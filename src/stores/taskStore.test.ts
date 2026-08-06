import { describe, expect, it } from "vitest";
import { adoptCreatedTaskState } from "./taskStore";
import type { TaskView } from "../types/task";

function task(status: TaskView["status"], id = "tsk-test"): TaskView {
  return {
    id,
    projectId: "prj_test",
    status,
    progress: { mode: "indeterminate" },
    createdAt: "2026-08-06T00:00:00Z",
    outputAssetIds: [],
  };
}

describe("adoptCreatedTaskState", () => {
  it("adopts the command response when the event stream has not seen the task", () => {
    const created = task("CREATED");
    const result = adoptCreatedTaskState({ recentTasks: [] }, created);

    expect(result.currentTask).toBe(created);
    expect(result.recentTasks).toEqual([created]);
  });

  it("keeps a RUNNING event instead of regressing to CREATED", () => {
    const running = task("RUNNING");
    const result = adoptCreatedTaskState(
      { currentTask: running, recentTasks: [running] },
      task("CREATED"),
    );

    expect(result.currentTask).toBe(running);
    expect(result.recentTasks).toEqual([running]);
  });

  it("keeps a SUCCEEDED event instead of regressing to CREATED", () => {
    const succeeded = task("SUCCEEDED");
    const result = adoptCreatedTaskState(
      { currentTask: succeeded, recentTasks: [succeeded] },
      task("CREATED"),
    );

    expect(result.currentTask).toBe(succeeded);
    expect(result.recentTasks).toEqual([succeeded]);
  });
});
