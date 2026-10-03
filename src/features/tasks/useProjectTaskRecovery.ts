import { useEffect, useRef, useState } from "react";
import type { TaskView } from "../../types/task";
import { useProjectStore } from "../../stores/projectStore";
import { useTaskStore } from "../../stores/taskStore";
import { toUserMessage } from "../../i18n/errorMessages";

/** Owns project task requests, not task data: TaskStore remains authoritative. */
export interface ProjectTaskRecoveryServices {
  listRecentTasks: (projectId: string, limit: number) => Promise<TaskView[]>;
  reconcileActiveTasks: () => Promise<{ examined: number; succeeded: number; deferred: number; unresolved: number }>;
}
export function useProjectTaskRecovery(activeProjectId: string | undefined, setError: (error: string | null) => void, services: ProjectTaskRecoveryServices) {
  const { listRecentTasks, reconcileActiveTasks } = services;
  const setRecentTasks = useTaskStore((state) => state.setRecentTasks);
  const recentTasks = useTaskStore((state) => state.recentTasks);
  const [projectContextLoading, setProjectContextLoading] = useState(false);
  const [reconciling, setReconciling] = useState(false);
  const [recoveryNotice, setRecoveryNotice] = useState<string | null>(null);
  const epoch = useRef(0);
  useEffect(() => {
    ++epoch.current;
    setReconciling(false);
    return () => { ++epoch.current; };
  }, [activeProjectId]);
  useEffect(() => {
    if (!activeProjectId) return;
    const requestedProjectId = activeProjectId;
    let cancelled = false;
    setProjectContextLoading(true);
    void listRecentTasks(requestedProjectId, 10)
      .then((tasks) => {
        if (!cancelled && useProjectStore.getState().activeProjectId === requestedProjectId) {
          setRecentTasks(tasks);
        }
      })
      .catch((loadError: unknown) => {
        if (!cancelled && useProjectStore.getState().activeProjectId === requestedProjectId) {
          setError(toUserMessage(loadError));
        }
      })
      .finally(() => {
        if (!cancelled) setProjectContextLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [activeProjectId, setRecentTasks, setError, listRecentTasks]);

  async function reconcileTasks() {
    if (!activeProjectId) return;
    const token = epoch.current;
    const requestedProjectId = activeProjectId;
    const current = () => token === epoch.current && useProjectStore.getState().activeProjectId === requestedProjectId;
    setReconciling(true);
    setRecoveryNotice(null);
    try {
      const report = await reconcileActiveTasks();
      const tasks = await listRecentTasks(requestedProjectId, 10);
      if (!current()) return;
      setRecentTasks(tasks);
      setRecoveryNotice(
        `已检查 ${report.examined} 个任务：${report.succeeded} 个已更新，${report.deferred} 个等待后续同步，${report.unresolved} 个状态未确定。`,
      );
    } catch (recoveryError: unknown) {
      if (current()) setRecoveryNotice(toUserMessage(recoveryError));
    } finally {
      if (token === epoch.current) setReconciling(false);
    }
  }

  return { recentTasks, projectContextLoading, reconciling, recoveryNotice, setRecoveryNotice, reconcileTasks };
}
