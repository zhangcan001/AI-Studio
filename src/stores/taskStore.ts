import { create } from "zustand";
import type { TaskView } from "../types/task";

interface TaskState {
  currentTask?: TaskView;
  recentTasks: TaskView[];
  setCurrentTask: (task?: TaskView) => void;
  setRecentTasks: (tasks: TaskView[]) => void;
  upsertTask: (task: TaskView) => void;
  adoptCreatedTask: (task: TaskView) => void;
}

export interface TaskStoreSnapshot {
  currentTask?: TaskView;
  recentTasks: TaskView[];
}

/**
 * Adopt the synchronous generation_create response without replacing a task
 * update that arrived through the event stream first.
 */
export function adoptCreatedTaskState(
  snapshot: TaskStoreSnapshot,
  createdTask: TaskView,
): TaskStoreSnapshot {
  const existing = snapshot.recentTasks.find((task) => task.id === createdTask.id);
  if (existing) {
    return {
      recentTasks: snapshot.recentTasks,
      currentTask: existing,
    };
  }

  return {
    recentTasks: [createdTask, ...snapshot.recentTasks].slice(0, 50),
    currentTask: createdTask,
  };
}

export const useTaskStore = create<TaskState>((set) => ({
  recentTasks: [],
  setCurrentTask: (currentTask) => set({ currentTask }),
  setRecentTasks: (recentTasks) =>
    set((state) => ({
      recentTasks,
      currentTask: state.currentTask ?? recentTasks[0],
    })),
  upsertTask: (task) =>
    set((state) => {
      const recentTasks = [task, ...state.recentTasks.filter((item) => item.id !== task.id)].slice(
        0,
        50,
      );
      return {
        recentTasks,
        currentTask: state.currentTask?.id === task.id ? task : state.currentTask ?? task,
      };
    }),
  adoptCreatedTask: (task) =>
    set((state) =>
      adoptCreatedTaskState(
        { recentTasks: state.recentTasks, currentTask: state.currentTask },
        task,
      ),
    ),
}));
