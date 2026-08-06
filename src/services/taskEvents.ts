import { listen, type Event, type UnlistenFn } from "@tauri-apps/api/event";
import type { TaskView } from "../types/task";

export const TASK_UPDATED_EVENT = "task://updated";

export function subscribeTaskUpdates(onUpdate: (task: TaskView) => void): Promise<UnlistenFn> {
  return listen<TaskView>(TASK_UPDATED_EVENT, (event: Event<TaskView>) => {
    onUpdate(normalizeTaskPayload(event.payload));
  });
}

function normalizeTaskPayload(payload: TaskView): TaskView {
  return {
    ...payload,
    outputAssetIds: payload.outputAssetIds ?? [],
  };
}
