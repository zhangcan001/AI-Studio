import { expect, it } from 'vitest';
// @ts-expect-error Node acceptance helper, excluded from application transport.
import { classifyIpcRequest, createIpcObserver } from '../../scripts/testing/native-ipc-observer.mjs';

it('phase11_ipc distinguishes preflight probes malformed payloads and real retry attempts', () => {
  const event = (method: string, body: string | undefined, requestId: string, command = 'product_run_list') => ({ method: 'Network.requestWillBeSent', params: { requestId, request: { url: `http://ipc.localhost/${command}`, method, postData: body } } });
  const observer = createIpcObserver();
  expect(observer.observe(event('OPTIONS', undefined, 'pre'))).toBe('PREFLIGHT');
  expect(observer.observe(event('POST', undefined, 'probe'))).toBe('PROBE');
  expect(observer.observe(event('POST', '{broken', 'bad'))).toBe('MALFORMED');
  expect(classifyIpcRequest(event('POST', 'null', 'null')).kind).toBe('MALFORMED');
  for (const command of ['product_creation_generate', 'task_list_recent', 'product_run_list', 'product_library_list', 'workflow_library_list']) {
    const call = event('POST', JSON.stringify({ projectId: 'a' }), `${command}-1`, command);
    expect(observer.observe(call)).toBe('COMMAND');
    observer.observe(call); // Duplicate event delivery, not a new call.
    expect(observer.count(command, 'a')).toBe(1);
    observer.observe(event('POST', JSON.stringify({ request: { projectId: 'a' } }), `${command}-retry`, command));
    expect(observer.count(command, 'a')).toBe(2); // Do not hide retries/duplicates.
    expect(observer.count(command, 'b')).toBe(0);
  }
  expect(observer.requests).toHaveLength(10);
  expect(classifyIpcRequest({ method: 'Network.responseReceived' }).kind).toBe('NON_IPC');
  const unrelated = event('POST', '{}', 'vite'); unrelated.params.request.url = 'http://localhost:1420/src/app/App.tsx';
  expect(classifyIpcRequest(unrelated).kind).toBe('NON_IPC');
});
