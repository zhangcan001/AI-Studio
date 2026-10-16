import { describe, expect, it } from "vitest";
import { parseShotPrompt, resolvePreset } from "./shotPromptParser";
import type { StoryEntity } from "../../types/storyEntity";

const ent = (id: string, name: string, kind: StoryEntity["kind"]) => ({ id, name, kind } as StoryEntity);
const presets = [
  { id: "mp-0_98-16x9", label: "0.98MP · 1344×768", width: 1344, height: 768 },
  { id: "mp-2_09-16x9", label: "2.09MP · 1920×1088", width: 1920, height: 1088 },
  { id: "mp-0_98-9x16", label: "0.98MP · 768×1344", width: 768, height: 1344 },
  { id: "mp-2_09-9x16", label: "2.09MP · 1088×1920", width: 1088, height: 1920 },
  { id: "mp-0_98-1x1", label: "0.98MP · 992×992", width: 992, height: 992 },
];

describe("parseShotPrompt", () => {
  it("recognizes duration in several spellings", () => {
    expect(parseShotPrompt("雨夜街头 5秒").durationSeconds).toBe(5);
    expect(parseShotPrompt("时长：8 秒钟").durationSeconds).toBe(8);
    expect(parseShotPrompt("a run 6s long").durationSeconds).toBe(6);
    expect(parseShotPrompt("十秒的推镜").durationSeconds).toBe(10);
    expect(parseShotPrompt("雨夜街头").durationSeconds).toBeUndefined();
  });
  it("recognizes aspect ratios and orientation words", () => {
    expect(parseShotPrompt("16:9 远景").aspect).toBe("16:9");
    expect(parseShotPrompt("画幅 9：16").aspect).toBe("9:16");
    expect(parseShotPrompt("竖屏 特写").aspect).toBe("9:16");
    expect(parseShotPrompt("横屏 全景").aspect).toBe("16:9");
    expect(parseShotPrompt("12:30 的钟楼").aspect).toBeUndefined();
  });
  it("recognizes resolutions", () => {
    expect(parseShotPrompt("720p").resolution).toEqual({ kind: "lines", lines: 720 });
    expect(parseShotPrompt("1080P 高清").resolution).toEqual({ kind: "lines", lines: 1080 });
    expect(parseShotPrompt("1344x768").resolution).toEqual({ kind: "size", width: 1344, height: 768 });
    expect(parseShotPrompt("768×1344").resolution).toEqual({ kind: "size", width: 768, height: 1344 });
  });
  it("recognizes shot numbers", () => {
    expect(parseShotPrompt("镜头3 推近").shotNumber).toBe(3);
    expect(parseShotPrompt("镜号：12").shotNumber).toBe(12);
    expect(parseShotPrompt("第三个镜头").shotNumber).toBe(3);
  });
  it("binds @names and exact names, longest match first", () => {
    const list = [ent("a", "小明", "character"), ent("b", "小明妈妈", "character"), ent("c", "雨夜街头", "scene"), ent("d", "红伞", "prop")];
    expect(parseShotPrompt("@小明妈妈 撑着红伞走过雨夜街头", list).entityIds.sort()).toEqual(["b", "c", "d"]);
    expect(parseShotPrompt("小明 和 @小明妈妈", list).entityIds.sort()).toEqual(["a", "b"]);
    expect(parseShotPrompt("无关文本", list).entityIds).toEqual([]);
  });
  it("never needs to modify the prompt and ignores plain numbers", () => {
    const p = parseShotPrompt("3 个人站在路口");
    expect(p).toEqual({ entityIds: [] });
  });
});

describe("resolvePreset", () => {
  it("maps 720p/1080p onto the megapixel ladder at the requested aspect", () => {
    expect(resolvePreset({ resolution: { kind: "lines", lines: 720 } }, presets)?.id).toBe("mp-0_98-16x9");
    expect(resolvePreset({ aspect: "9:16", resolution: { kind: "lines", lines: 1080 } }, presets)?.id).toBe("mp-2_09-9x16");
  });
  it("keeps the current size tier when only the aspect changes", () => {
    expect(resolvePreset({ aspect: "9:16" }, presets, { width: 1920, height: 1088 })?.id).toBe("mp-2_09-9x16");
    expect(resolvePreset({ aspect: "1:1" }, presets)?.id).toBe("mp-0_98-1x1");
  });
  it("uses exact sizes and snaps unsupported ones to the nearest preset", () => {
    expect(resolvePreset({ resolution: { kind: "size", width: 768, height: 1344 } }, presets)?.id).toBe("mp-0_98-9x16");
    expect(resolvePreset({ resolution: { kind: "size", width: 1280, height: 720 } }, presets)?.id).toBe("mp-0_98-16x9");
  });
  it("returns nothing without presets or without input", () => {
    expect(resolvePreset({ aspect: "16:9" }, [])).toBeUndefined();
    expect(resolvePreset({}, presets)).toBeUndefined();
  });
});
