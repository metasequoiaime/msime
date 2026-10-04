import type { Snapshot, Preferences } from "../index";
import type { OnboardingChoices, OnboardingInputScheme } from "./onboarding-page";

/** Applies the preferences selected by the first-launch flow to a loaded snapshot. */
export function completeOnboardingPreferences(
  snapshot: Snapshot,
  scheme: OnboardingInputScheme,
  choices: Pick<OnboardingChoices, "candidateEnglishGloss">,
): Preferences {
  const inputScheme = scheme === "wubi" ? "wubi" : "quanpin";
  const enabled = [...(snapshot.preferences.touch_keyboard_schemes?.enabled ?? [])];
  if (!enabled.includes(scheme)) enabled.push(scheme);

  return {
    ...snapshot.preferences,
    ...(choices.candidateEnglishGloss === undefined
      ? {}
      : { candidate_english_gloss: choices.candidateEnglishGloss }),
    // 五笔键盘对应五笔方案，全拼 26 键和 9 键都对应全拼。
    scheme: inputScheme,
    last_chinese_scheme: inputScheme,
    touch_keyboard_layout: scheme === "nine_key" ? "nine_key" : "twenty_six_key",
    touch_keyboard_schemes: {
      ...snapshot.preferences.touch_keyboard_schemes,
      enabled,
      selected: scheme,
    },
  };
}
