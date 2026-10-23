import { describe, expect, it } from "vitest";
import { ENGLISH_PROMPT_HINT, englishPromptHint } from "./promptLanguage";

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
