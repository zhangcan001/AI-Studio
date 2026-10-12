import { afterEach, expect, it, vi } from "vitest";
import { invalidateVideoInputs, resetVideoInputInvalidationForTests, subscribeVideoInputInvalidation } from "./videoInputInvalidation";

afterEach(() => {
  resetVideoInputInvalidationForTests();
});

it("notifies matching subscribers and supports unsubscribe", () => {
  const a = vi.fn();
  const b = vi.fn();
  const offA = subscribeVideoInputInvalidation(a);
  const offB = subscribeVideoInputInvalidation(b);
  invalidateVideoInputs("p1");
  expect(a).toHaveBeenCalledWith("p1");
  expect(b).toHaveBeenCalledWith("p1");
  offA();
  invalidateVideoInputs("p2");
  expect(a).toHaveBeenCalledTimes(1);
  expect(b).toHaveBeenLastCalledWith("p2");
  offB();
  invalidateVideoInputs("p3");
  expect(b).toHaveBeenCalledTimes(2);
});
