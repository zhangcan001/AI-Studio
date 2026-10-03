// Test/acceptance-only CDP observer. No application transport interception.
export function classifyIpcRequest(event) {
  const request = event?.params?.request;
  if (event?.method !== 'Network.requestWillBeSent' || !request) return { kind: 'NON_IPC' };
  let url;
  try { url = new URL(request.url); } catch { return { kind: 'NON_IPC' }; }
  if (url.hostname !== 'ipc.localhost') return { kind: 'NON_IPC' };
  if (request.method === 'OPTIONS') return { kind: 'PREFLIGHT' };
  if (!request.postData) return { kind: 'PROBE' };
  if (request.method !== 'POST') return { kind: 'PROBE' };
  try {
    const args = JSON.parse(request.postData);
    if (!args || typeof args !== 'object' || Array.isArray(args)) return { kind: 'MALFORMED' };
    return { kind: 'COMMAND', requestId: event.params.requestId, command: decodeURIComponent(url.pathname.slice(1)), args };
  } catch { return { kind: 'MALFORMED' }; }
}

export function createIpcObserver() {
  const requests = [], seen = new Set();
  return {
    observe(event) {
      const entry = classifyIpcRequest(event);
      // A duplicate CDP delivery is not another invocation. New IDs (including
      // retries) stay separate; identical command/args must never be collapsed.
      if (entry.kind === 'COMMAND' && (!entry.requestId || !seen.has(entry.requestId))) {
        if (entry.requestId) seen.add(entry.requestId);
        requests.push(entry);
      }
      return entry.kind;
    },
    count(command, projectId) { return requests.filter(r => r.command === command && (r.args.projectId ?? r.args.request?.projectId) === projectId).length; },
    requests,
  };
}
