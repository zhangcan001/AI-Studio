import { describe, expect, it } from "vitest";
import { generatorLabel, packageKind, packageOptionLabel, packageVariantHint } from "./generatorPresentation";
import type { GeneratorOption } from "./types";

function opt(name: string, mode = "REF2VA"): GeneratorOption {
  return {
    selectionRef: name,
    name,
    version: "1.0.0",
    mode,
    mediaKind: "video",
    availability: true,
    availabilityReason: null,
    recommended: false,
    fields: [],
  };
}

describe("package presentation", () => {
  it("classifies Pika / Veda / Standard by product name", () => {
    expect(packageKind(opt("MiniMax H3 Pika 全能参考 · 8步"))).toBe("pika");
    expect(packageKind(opt("MiniMax H3 Veda Turbo"))).toBe("veda");
    expect(packageKind(opt("MiniMax H3 Standard 全能参考 · 20步"))).toBe("standard");
    expect(packageVariantHint(opt("anything veda turbo"))).toContain("易白闪，成片慎用");
    expect(packageOptionLabel(opt("MiniMax H3 Pika 全能参考 · 8步"), 0)).toContain("8步加速参考");
  });

  it("keeps technical workflow ids readable without changing selectionRef", () => {
    expect(generatorLabel(opt("wfl_minimax_h3_reference_pika_8step"), 0)).toContain("H3 Pika");
    expect(generatorLabel(opt("wfl_minimax_h3_reference_veda_turbo"), 0)).toContain("H3 Veda");
  });
});
