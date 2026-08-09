import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getRuntimeActivityStatus } from "../services/tauriClient";
import { shouldWarnBeforeClose } from "./shouldWarnBeforeClose";

const ACTIVE_EXIT_WARNING =
  "当前仍有生成任务或生产队列正在运行。\n\n退出不会取消任务，但关闭期间将无法显示实时进度。确定退出吗？";
const UNKNOWN_EXIT_WARNING =
  "暂时无法确认任务状态。为保护正在运行的任务，是否仍要退出？";

export function useSafeExit() {
  useEffect(() => {
    let disposed = false;
    const windowHandle = getCurrentWindow();
    let unlisten: (() => void) | undefined;

    const registration = windowHandle.onCloseRequested(async (event) => {
      let activity;
      let queryFailed = false;
      try {
        activity = await getRuntimeActivityStatus();
      } catch {
        queryFailed = true;
      }
      if (disposed) return;

      const shouldWarn = shouldWarnBeforeClose(activity, queryFailed);
      const confirmed = !shouldWarn || window.confirm(queryFailed ? UNKNOWN_EXIT_WARNING : ACTIVE_EXIT_WARNING);
      if (!confirmed) event.preventDefault();
    });

    void registration.then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
