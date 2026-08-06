import { create } from "zustand";
import type { TaskView } from "../types/task";

interface TaskState {
  currentTask?: TaskView;
  recentTasks: TaskView[];
  setCurrentTask: (task?: TaskView) => void;
  setRecentTasks: (tasks: TaskView[]) => void;
  upsertTask: (task: TaskView) => void;
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
}));
