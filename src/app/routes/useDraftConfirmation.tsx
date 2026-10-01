import { useCallback, useEffect, useRef, useState } from "react";

/** A UI decision only: no route/draft data or persistence is owned here. */
export function useDraftConfirmation() {
  const [message, setMessage] = useState<string>();
  const pending = useRef<((accepted: boolean) => void) | undefined>(undefined);
  const element = useRef<HTMLDialogElement>(null);
  const confirm = useCallback((nextMessage: string) => {
    // Ignore competing navigation while the user is deciding the first request.
    if (pending.current) return Promise.resolve(false);
    return new Promise<boolean>((resolve) => { pending.current = resolve; setMessage(nextMessage); });
  }, []);
  const finish = useCallback((accepted: boolean) => {
    const resolve = pending.current;
    pending.current = undefined;
    element.current?.close();
    setMessage(undefined);
    resolve?.(accepted);
  }, []);
  useEffect(() => {
    if (message && element.current && !element.current.open) element.current.showModal();
  }, [message]);
  useEffect(() => () => { pending.current?.(false); }, []);
  const dialog = message ? <dialog ref={element} className="route-draft-confirmation" aria-labelledby="route-draft-title" onCancel={(event) => { event.preventDefault(); finish(false); }}>
    <h2 id="route-draft-title">未保存的修改</h2>
    <p>{message}</p>
    <div><button type="button" autoFocus onClick={() => finish(false)}>继续编辑</button><button type="button" onClick={() => finish(true)}>放弃修改并继续</button></div>
  </dialog> : null;
  return { confirm, dialog };
}
