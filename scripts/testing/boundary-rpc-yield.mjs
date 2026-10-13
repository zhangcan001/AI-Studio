import { afterEach } from "vitest";
import { setImmediate } from "node:timers";

// Historical proof tests perform long synchronous replay. Give the worker RPC
// channel an event-loop turn between tests so completed assertions can be
// acknowledged. No assertions, timeouts, test selection or validation are changed.
afterEach(() => new Promise(resolve => setImmediate(resolve)));
