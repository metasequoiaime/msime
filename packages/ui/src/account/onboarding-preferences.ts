import type { Snapshot, Preferences } from "../index";
import type { OnboardingChoices, OnboardingInputScheme } from "./onboarding-page";

/** Applies the preferences selected by the first-launch flow to a loaded snapshot. */
export function completeOnboardingPreferences(
  snapshot: Snapshot,
  scheme: OnboardingInputScheme,
  choices: Pick<OnboardingChoices, "candidateEnglishGloss">,
): Preferences {
  const enabled = [...(snapshot.preferences.touch_keyboard_schemes?.enabled ?? [])];
  if (!enabled.includes(scheme)) enabled.push(scheme);

  return {
    ...snapshot.preferences,
    ...(choices.candidateEnglishGloss === undefined
      ? {}
      : { candidate_english_gloss: choices.candidateEnglishGloss }),
    scheme: "quanpin",
    last_chinese_scheme: "quanpin",
    touch_keyboard_layout: scheme === "nine_key" ? "nine_key" : "twenty_six_key",
    touch_keyboard_schemes: {
      ...snapshot.preferences.touch_keyboard_schemes,
      enabled,
      selected: scheme,
    },
  };
}
