import { describe, expect, it } from "vitest";
import { MAX_SHOT_DURATION_SECONDS, shotDurationIssue, shotDurationMax, editableDurationIssue } from "./shotDuration";

describe("shot duration cap", () => {
  it("separates nonnumeric, integer, recipe minimum, recipe maximum and product limit errors", () => {
    expect(editableDurationIssue(NaN)).toBe("时长必须是有效数字。");
    expect(editableDurationIssue(Infinity)).toBe("时长必须是有效数字。");
    expect(editableDurationIssue(1.5,{type:"integer"})).toBe("时长必须是整数秒。");
    expect(editableDurationIssue(3,{min:4})).toContain("最短 4 秒");
    expect(editableDurationIssue(10,{max:9})).toContain("最长 9 秒");
    expect(editableDurationIssue(12,{max:15})).toContain("最长 11 秒");
    expect(editableDurationIssue(undefined,{required:true})).toContain("请填写");
    expect(editableDurationIssue(11,{min:4,max:15})).toBeUndefined();
  });
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
