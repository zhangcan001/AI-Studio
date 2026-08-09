import { describe, expect, it } from "vitest";
import { shouldWarnBeforeClose } from "./shouldWarnBeforeClose";

describe("安全退出判断", () => {
  it("does not warn when the global runtime is idle", () => {
    expect(shouldWarnBeforeClose({ activeTaskCount: 0, productionBusy: false })).toBe(false);
  });

  it("warns for active tasks or a busy production queue", () => {
    expect(shouldWarnBeforeClose({ activeTaskCount: 1, productionBusy: false })).toBe(true);
    expect(shouldWarnBeforeClose({ activeTaskCount: 0, productionBusy: true })).toBe(true);
  });

  it("warns conservatively when the activity query fails", () => {
    expect(shouldWarnBeforeClose(undefined, true)).toBe(true);
  });
});
