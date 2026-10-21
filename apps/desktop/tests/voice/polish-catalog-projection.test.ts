import { expect, test, vi } from "vitest";

vi.mock("../../../../packages/ui/src/voice/polish-prompt-catalog.json", () => ({
  default: [
    { id: "cleanup", title: "合成整理", prompt: "synthetic cleanup prompt" },
    { id: "faithful", title: "合成校对", prompt: "synthetic faithful prompt" },
    { id: "zh2en", title: "合成翻译", prompt: "synthetic translation prompt" },
    { id: "casual", title: "合成口语", prompt: "synthetic casual prompt" },
  ],
}));

import {
  POLISH_PRESET_IDS,
  POLISH_PRESET_NAMES,
  POLISH_PRESETS,
  polishPresetPrompt,
  polishPromptFor,
} from "../../../../packages/ui/src/voice/polish-presets";

test("内置名称与正文从目录投影，槽位和未知方案仍由原接口处理", () => {
  expect(POLISH_PRESET_IDS).toEqual(["cleanup", "faithful", "zh2en", "casual"]);
  expect(POLISH_PRESET_NAMES).toEqual({
    cleanup: "合成整理",
    faithful: "合成校对",
    zh2en: "合成翻译",
    casual: "合成口语",
  });
  expect(POLISH_PRESETS).toEqual({
    cleanup: "synthetic cleanup prompt",
    faithful: "synthetic faithful prompt",
    zh2en: "synthetic translation prompt",
    casual: "synthetic casual prompt",
  });
  expect(polishPresetPrompt("faithful")).toBe("synthetic faithful prompt");
  expect(polishPresetPrompt("unknown")).toBe("");
  expect(polishPromptFor("custom_2", { polish_prompt_custom_2: "synthetic custom" })).toBe(
    "synthetic custom",
  );
});
