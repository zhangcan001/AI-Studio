// CDP addScriptToEvaluateOnNewDocument ONLY. Never part of the app bundle.
// Counts instrumented registrations, not heap reachability or all JS subscriptions.
(() => {
  if (window.__phase12Acceptance) return;
  if (window.__REACT_DEVTOOLS_GLOBAL_HOOK__) throw Error('Existing DevTools hook; do not replace it');
  const commits = [];
  let renderer = 0;
  window.__REACT_DEVTOOLS_GLOBAL_HOOK__ = {
    supportsFiber: true,
    inject() { return ++renderer; },
    onScheduleFiberRoot() {}, onCommitFiberUnmount() {}, onPostCommitFiberRoot() {},
    onCommitFiberRoot(id, root) {
      const owners = new Set();
      let performedWorkFibers = 0;
      function visit(fiber) {
        if (!fiber) return;
        if (fiber.flags & 1) performedWorkFibers++;
        const type = fiber.type;
        const name = typeof type === 'function' ? type.displayName ?? type.name : '';
        if (['App', 'AppShellV3', 'CreatePage', 'RunsPage', 'LibraryPage'].includes(name)) owners.add(name);
        visit(fiber.child); visit(fiber.sibling);
      }
      visit(root.current);
      commits.push({ atMs: performance.now(), rendererId: id, owners: [...owners], performedWorkFibers });
    },
  };
  const add = EventTarget.prototype.addEventListener;
  const remove = EventTarget.prototype.removeEventListener;
  const targets = new WeakMap(), activeListeners = new Set();
  EventTarget.prototype.addEventListener = function(type, listener, options) {
    if (!listener || options?.signal?.aborted) return add.call(this, type, listener, options);
    const capture = typeof options === 'boolean' ? options : Boolean(options?.capture);
    let registrations = targets.get(this);
    if (!registrations) targets.set(this, registrations = new Map());
    const key = `${type}:${capture}`;
    let callbacks = registrations.get(key);
    if (!callbacks) registrations.set(key, callbacks = new Map());
    let record = callbacks.get(listener);
    if (!record) {
      record = { type, capture, wrapped: undefined, target: new WeakRef(this), tracking: undefined };
      // Do not keep detached DOM alive merely to count its registrations.
      record.tracking = new WeakRef(record);
      const cleanup = () => { activeListeners.delete(record.tracking); callbacks.delete(listener); };
      record.wrapped = function(event) {
        if (typeof options === 'object' && options.once) cleanup();
        return typeof listener === 'function' ? listener.call(this, event) : listener.handleEvent(event);
      };
      callbacks.set(listener, record); activeListeners.add(record.tracking);
      if (options?.signal) add.call(options.signal, 'abort', cleanup, { once: true });
    }
    return add.call(this, type, record.wrapped, options);
  };
  EventTarget.prototype.removeEventListener = function(type, listener, options) {
    const capture = typeof options === 'boolean' ? options : Boolean(options?.capture);
    const callbacks = targets.get(this)?.get(`${type}:${capture}`);
    const record = callbacks?.get(listener);
    if (record) { callbacks.delete(listener); activeListeners.delete(record.tracking); }
    return remove.call(this, type, record?.wrapped ?? listener, options);
  };
  const intervals = new Map(), timeouts = new Map();
  const setIntervalNative = window.setInterval, clearIntervalNative = window.clearInterval;
  const setTimeoutNative = window.setTimeout, clearTimeoutNative = window.clearTimeout;
  window.setInterval = function(callback, ms, ...args) {
    const id = setIntervalNative.call(window, callback, ms, ...args); intervals.set(id, ms); return id;
  };
  window.setTimeout = function(callback, ms, ...args) {
    if (typeof callback !== 'function') return setTimeoutNative.call(window, callback, ms, ...args);
    const id = setTimeoutNative.call(window, function(...values) {
      timeouts.delete(id); return callback.apply(this, values);
    }, ms, ...args);
    timeouts.set(id, ms); return id;
  };
  window.clearInterval = function(id) { intervals.delete(id); timeouts.delete(id); return clearIntervalNative.call(window, id); };
  window.clearTimeout = function(id) { intervals.delete(id); timeouts.delete(id); return clearTimeoutNative.call(window, id); };
  const objectUrls = new Set(), createUrl = URL.createObjectURL, revokeUrl = URL.revokeObjectURL;
  URL.createObjectURL = function(blob) { const url = createUrl.call(URL, blob); objectUrls.add(url); return url; };
  URL.revokeObjectURL = function(url) { objectUrls.delete(url); return revokeUrl.call(URL, url); };
  window.__phase12Acceptance = {
    resetCommits() { commits.length = 0; },
    snapshot() {
      let connected = 0, detached = 0, globals = 0;
      const byType = {};
      for (const reference of activeListeners) {
        const record = reference.deref(), target = record?.target.deref();
        if (!record || !target) { activeListeners.delete(reference); continue; }
        if (target instanceof Node && !target.isConnected) { detached++; continue; }
        connected++; byType[record.type] = (byType[record.type] ?? 0) + 1;
        if (target === window || target === document) globals++;
      }
      return { renderMethod: 'production React DevTools root-commit callback; not actualDuration profiling build',
        rendererCount: renderer, rootCommits: commits.length, commits: commits.slice(),
        resources: { registeredEventListeners: connected, globalEventListeners: globals,
          connectedListenersByType: byType, detachedElementRegistrations: detached,
          detachedInterpretation: 'GC is uncontrolled; not a live-owner leak count', activeIntervals: intervals.size,
          intervalPeriodsMs: [...intervals.values()].sort((a, b) => a - b), activeTimeouts: timeouts.size,
          timeoutPeriodsMs: [...timeouts.values()].sort((a, b) => a - b),
          ownedObjectUrls: objectUrls.size, jsNotificationSubscriptions: 'NOT_MEASURED' } };
    },
  };
})();
