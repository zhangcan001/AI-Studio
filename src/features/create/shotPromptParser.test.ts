import { describe, expect, it } from "vitest";
import { explicitEntityMentions, parseShotPrompt, resolvePreset, resolvePromptSettings } from "./shotPromptParser";
import type { StoryEntity } from "../../types/storyEntity";

const ent = (id: string, name: string, kind: StoryEntity["kind"]) => ({ id, name, kind } as StoryEntity);

it("resolves only explicit unique @names, longest first, in occurrence order", () => {
  const hero = ent("hero", "小明", "character"), mother = ent("mother", "小明妈妈", "character"), scene = ent("scene", "太空", "scene");
  expect(explicitEntityMentions("@小明妈妈在@太空中飞翔，@小明妈妈看小明", [hero, mother, scene])).toEqual({ entities: [mother, scene], error: undefined });
  expect(explicitEntityMentions("小明在太空", [hero, scene]).entities).toEqual([]);
  expect(explicitEntityMentions("@不存在", [hero]).error).toBeTruthy();
  expect(explicitEntityMentions("@小明", [hero, ent("other", "小明", "character")]).error).toContain("同名");
  expect(explicitEntityMentions("@AnnaBelle", [ent("anna", "Anna", "character")]).error).toBeTruthy();
});
const presets = [
  { id: "mp-0_98-16x9", label: "0.98MP · 1344×768", width: 1344, height: 768 },
  { id: "mp-2_09-16x9", label: "2.09MP · 1920×1088", width: 1920, height: 1088 },
  { id: "mp-0_98-9x16", label: "0.98MP · 768×1344", width: 768, height: 1344 },
  { id: "mp-2_09-9x16", label: "2.09MP · 1088×1920", width: 1088, height: 1920 },
  { id: "mp-0_98-1x1", label: "0.98MP · 992×992", width: 992, height: 992 },
];

describe("parseShotPrompt", () => {
  it.each([
    ["总像素0.5MP", 524288], ["0.98mp", 1027604], ["０．５ＭＰ", 524288],
    ["50万像素", 500000], ["总像素：500000", 500000], ["总像素50万", 500000],
    ["0.5百万像素", 500000], ["1,000,000像素", 1000000],
    ["16:9，0.5MP", 524288], ["总像素500000，16:9", 500000],
  ])("recognizes explicit total pixels in %s without confusing Comfy MP and decimal pixels", (text, pixels) => {
    expect(parseShotPrompt(text).resolution).toEqual({ kind: "pixels", pixels });
  });
  it.each(["画幅1920", "500MB", "预算50万", "12.5MP4"])("does not guess a pixel budget from %s", text => {
    expect(parseShotPrompt(text).resolution).toBeUndefined();
  });
  it.each(["0MP", "-0.5MP", "999999999999999999999像素"])("rejects invalid pixel budgets %s", text => {
    expect(parseShotPrompt(text).resolution).toBeUndefined();
    expect(parseShotPrompt(text).warnings?.join(" ")).toContain("总像素");
  });
  it("uses the last explicit total and retains the existing full-dimension priority", () => {
    expect(parseShotPrompt("0.5MP，总像素100万").resolution).toEqual({ kind: "pixels", pixels: 1000000 });
    expect(parseShotPrompt("总像素100万，0.98MP").resolution).toEqual({ kind: "pixels", pixels: 1027604 });
    expect(parseShotPrompt("总像素0.5MP，1344×768").resolution).toEqual({ kind: "size", width: 1344, height: 768 });
  });
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
  it("reports incomplete dimensions without guessing a missing height or aspect", () => {
    const parsed = parseShotPrompt("画幅1920");
    expect(parsed.aspect).toBeUndefined();
    expect(parsed.resolution).toBeUndefined();
    expect(parsed.warnings).toEqual([expect.stringContaining("画幅/分辨率信息不完整")]);
    expect(parseShotPrompt("画幅1920×1080").warnings).toBeUndefined();
    expect(parseShotPrompt("分辨率1080p").warnings).toBeUndefined();
  });
});

describe("resolvePreset", () => {
  it("maps a total-pixel target through approved presets while retaining the saved aspect", () => {
    expect(resolvePreset(parseShotPrompt("0.98MP"), presets)?.id).toBe("mp-0_98-16x9");
    expect(resolvePreset(parseShotPrompt("0.98MP"), presets, { width: 1088, height: 1920 })?.id).toBe("mp-0_98-9x16");
    expect(resolvePreset(parseShotPrompt("总像素100万"), presets, { width: 992, height: 992 })?.id).toBe("mp-0_98-1x1");
    expect(resolvePreset(parseShotPrompt("0.98MP 21:9"), presets)).toBeUndefined();
  });
  it("preserves the backend nominal aspect even when aligned pixels drift beyond the old tolerance", () => {
    const aligned = [...presets, { id: "mp-0_5-4x3", width: 832, height: 640 }];
    expect(resolvePreset(parseShotPrompt("0.5MP"), aligned, { width: 832, height: 640 })?.id).toBe("mp-0_5-4x3");
    expect(resolvePreset(parseShotPrompt("0.5MP"), [])).toBeUndefined();
  });
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

describe("resolvePromptSettings", () => {
  it("uses selected Recipe bounds and presets, never creates arbitrary dimensions", () => {
    const generator = { fields: [{ type: "integer", key: "duration_seconds", label: "duration", required: true, min: 4, max: 10 }] as const, resolutionPresets: presets };
    const settings = resolvePromptSettings(parseShotPrompt("时长20秒 9:16 1080p"), generator, {});
    expect(settings.duration).toEqual({ type: "integer", value: 10 });
    expect(settings.preset?.id).toBe("mp-2_09-9x16");
    expect(settings.warnings).toEqual([]);
  });
  it("caps prompt durations at the 11 s product limit and says so", () => {
    const generator = { fields: [{ type: "integer", key: "duration_seconds", label: "duration", required: true, min: 4, max: 15 }] as const, resolutionPresets: presets };
    const settings = resolvePromptSettings(parseShotPrompt("时长14秒"), generator, {});
    expect(settings.duration).toEqual({ type: "integer", value: 11 });
    expect(settings.warnings).toEqual([expect.stringContaining("显存限制")]);
  });
  it("explains unsupported settings and leaves missing Recipe fields untouched", () => {
    const settings = resolvePromptSettings(parseShotPrompt("21:9 5秒"), { fields: [], resolutionPresets: presets }, {});
    expect(settings.duration).toBeUndefined();
    expect(settings.preset).toBeUndefined();
    expect(settings.warnings).toEqual([expect.stringContaining("没有时长参数"), expect.stringContaining("没有匹配的画幅")]);
  });
});
