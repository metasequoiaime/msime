import { describe, expect, test } from "vitest";
import type { Snapshot } from "@msime/ui";
import { completeOnboardingPreferences } from "../../../../packages/ui/src/account/onboarding-preferences";

const snapshot: Snapshot = {
  format_version: 1,
  revision: 12,
  preferences: {
    scheme: "wubi",
    shuangpin_profile: "xiaohe",
    candidate_page_size: 9,
    learning: false,
    chinese_punctuation: true,
    candidate_english_gloss: true,
    touch_keyboard_layout: "handwriting",
    touch_keyboard_schemes: {
      enabled: ["quanpin", "nine_key"],
      selected: "quanpin",
    },
    touch_voice_shortcut: true,
  },
};

describe("completeOnboardingPreferences", () => {
  test("adds a newly selected scheme and writes nine-key preferences", () => {
    const preferences = completeOnboardingPreferences(snapshot, "nine_key", {
      candidateEnglishGloss: false,
    });

    expect(preferences).toMatchObject({
      candidate_english_gloss: false,
      scheme: "quanpin",
      last_chinese_scheme: "quanpin",
      touch_keyboard_layout: "nine_key",
      touch_voice_shortcut: true,
      touch_keyboard_schemes: {
        enabled: ["quanpin", "nine_key"],
        selected: "nine_key",
      },
    });
    expect(snapshot.preferences).toEqual({
      scheme: "wubi",
      shuangpin_profile: "xiaohe",
      candidate_page_size: 9,
      learning: false,
      chinese_punctuation: true,
      candidate_english_gloss: true,
      touch_keyboard_layout: "handwriting",
      touch_keyboard_schemes: {
        enabled: ["quanpin", "nine_key"],
        selected: "quanpin",
      },
      touch_voice_shortcut: true,
    });
  });

  test("preserves an untouched gloss choice and does not duplicate an enabled scheme", () => {
    const preferences = completeOnboardingPreferences(snapshot, "quanpin", {});

    expect(preferences.candidate_english_gloss).toBe(true);
    expect(preferences.touch_keyboard_layout).toBe("twenty_six_key");
    expect(preferences.touch_keyboard_schemes).toEqual({
      enabled: ["quanpin", "nine_key"],
      selected: "quanpin",
    });
  });

  test("the 双拼 card selects Xiaohe shuangpin on the 26-key keyboard", () => {
    const preferences = completeOnboardingPreferences(snapshot, "xiaohe", {});

    expect(preferences).toMatchObject({
      scheme: "shuangpin",
      last_chinese_scheme: "shuangpin",
      shuangpin_profile: "xiaohe",
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes: { enabled: ["quanpin", "nine_key", "xiaohe"], selected: "xiaohe" },
    });
  });

  test("the 双拼 card keeps a shuangpin layout already in use", () => {
    const preferences = completeOnboardingPreferences(
      {
        ...snapshot,
        preferences: {
          ...snapshot.preferences,
          scheme: "shuangpin",
          shuangpin_profile: "ziranma",
          touch_keyboard_schemes: { enabled: ["ziranma"], selected: "ziranma" },
        },
      },
      "xiaohe",
      {},
    );

    expect(preferences).toMatchObject({
      scheme: "shuangpin",
      shuangpin_profile: "ziranma",
      touch_keyboard_schemes: { enabled: ["ziranma"], selected: "ziranma" },
    });
  });

  test("leaving before choosing a keyboard keeps the scheme and still saves the gloss", () => {
    const preferences = completeOnboardingPreferences(snapshot, "quanpin", {
      candidateEnglishGloss: false,
      keepScheme: true,
    });

    expect(preferences).toEqual({ ...snapshot.preferences, candidate_english_gloss: false });
  });

  test("the wubi edition's keyboard selects the Wubi scheme", () => {
    const preferences = completeOnboardingPreferences(
      {
        ...snapshot,
        preferences: {
          ...snapshot.preferences,
          scheme: "quanpin",
          touch_keyboard_schemes: { enabled: ["wubi", "handwriting"], selected: "handwriting" },
        },
      },
      "wubi",
      {},
    );

    expect(preferences).toMatchObject({
      scheme: "wubi",
      last_chinese_scheme: "wubi",
      touch_keyboard_layout: "twenty_six_key",
      touch_keyboard_schemes: { enabled: ["wubi", "handwriting"], selected: "wubi" },
    });
  });
});
