import { describe, expect, it } from "vitest";
import { ENGLISH_PROMPT_HINT, HEAVY_REFERENCE_HINT, appearanceLanguageHint, englishPromptHint, h3PromptHints, isLegacyCompiledPrompt } from "./promptLanguage";

describe("englishPromptHint", () => {
  it("hints when the description is mostly Chinese", () => {
    expect(englishPromptHint("@沈簿站在门口，镜头缓慢推进，8秒")).toBe(ENGLISH_PROMPT_HINT);
  });
  it("stays silent for English descriptions with Chinese dialogue or mentions", () => {
    expect(englishPromptHint("@沈簿 walks to the desk and says <d>[Chinese] 你来晚了，我们等了很久。</d>")).toBeUndefined();
    expect(englishPromptHint("@林舟 sits behind the table, camera pushes in slowly, “你好啊朋友”")).toBeUndefined();
    expect(englishPromptHint("")).toBeUndefined();
  });
});

describe("h3PromptHints", () => {
  it("lists every non-blocking hint", () => {
    const hints = h3PromptHints({ description: "@A walks. He says “你好啊”. [Shot 2] At 00:12.000, the shot cuts.", durationSeconds: 11, referenceCount: 7, unmentionedNames: ["书房"] });
    expect(hints.some(h => h.includes("350–500"))).toBe(true);
    expect(hints).toContain("对白建议写成 <d>[Chinese] 台词。</d> 格式");
    expect(hints.some(h => h.startsWith("切镜时间 00:12.000"))).toBe(true);
    expect(hints).toContain("以下参考未在描述中 @ 到：书房");
    expect(hints).toContain(HEAVY_REFERENCE_HINT);
  });
  it("stays quiet for a complete English description", () => {
    const description = Array.from({ length: 360 }, () => "word").join(" ") + " <d>[Chinese] 你好。</d>";
    expect(h3PromptHints({ description, durationSeconds: 8, referenceCount: 3, unmentionedNames: [] })).toEqual([]);
  });
});

describe("isLegacyCompiledPrompt", () => {
  it("flags old compiled formats only", () => {
    expect(isLegacyCompiledPrompt("subject_definitions:\n<Subject 1>: composition_reference - x")).toBe(true);
    expect(isLegacyCompiledPrompt("subject_definitions:\nx\n\ndetailed_description:\nA.\n[Shot 1] y")).toBe(false);
    expect(isLegacyCompiledPrompt("plain prompt")).toBe(false);
  });
  it("hints for non-English appearance", () => {
    expect(appearanceLanguageHint("三十岁女性，短发")).toBeTruthy();
    expect(appearanceLanguageHint("a tall man in a grey coat")).toBeUndefined();
  });
});
