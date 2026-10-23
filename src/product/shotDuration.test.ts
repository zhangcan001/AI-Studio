import { describe, expect, it } from "vitest";
import { MAX_SHOT_DURATION_SECONDS, shotDurationIssue, shotDurationMax } from "./shotDuration";

describe("shot duration cap", () => {
  it("caps at 11 seconds", () => {
    expect(MAX_SHOT_DURATION_SECONDS).toBe(11);
    expect(shotDurationMax(15)).toBe(11);
    expect(shotDurationMax(8)).toBe(8);
    expect(shotDurationMax(undefined)).toBe(11);
  });
  it("blocks over-limit shots with a Chinese message", () => {
    expect(shotDurationIssue(11)).toBeUndefined();
    expect(shotDurationIssue(undefined)).toBeUndefined();
    expect(shotDurationIssue(12)).toContain("镜头时长最长 11 秒（显存限制）");
  });
});
