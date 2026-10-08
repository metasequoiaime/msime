import type { Snapshot, Preferences } from "../index";
import type { TouchKeyboardScheme } from "../settings/touch-keyboard-scheme-helpers";
import type { OnboardingChoices, OnboardingInputScheme } from "./onboarding-page";

/** Applies the preferences selected by the first-launch flow to a loaded snapshot. */
export function completeOnboardingPreferences(
  snapshot: Snapshot,
  scheme: OnboardingInputScheme,
  choices: Pick<OnboardingChoices, "candidateEnglishGloss" | "keepScheme">,
): Preferences {
  const gloss =
    choices.candidateEnglishGloss === undefined
      ? {}
      : { candidate_english_gloss: choices.candidateEnglishGloss };
  // 在选择键盘之前「跳过」，保存的方案保持不变。
  if (choices.keepScheme) return { ...snapshot.preferences, ...gloss };

  const current = snapshot.preferences;
  // 双拼卡片的意思是「用双拼」：已经在用某种双拼时沿用它，只有第一次选双拼才落在小鹤，与 Android 的 `OnboardingChoicePolicy.shuangpinCard` 一致。
  const shuangpinProfile =
    scheme === "xiaohe" && current.scheme === "shuangpin" ? current.shuangpin_profile : "xiaohe";
  const selected: TouchKeyboardScheme = scheme === "xiaohe" ? shuangpinProfile : scheme;
  // 五笔键盘对应五笔方案，双拼键盘对应双拼方案，全拼 26 键和 9 键都对应全拼。
  const inputScheme = scheme === "wubi" ? "wubi" : scheme === "xiaohe" ? "shuangpin" : "quanpin";
  const enabled = [...(current.touch_keyboard_schemes?.enabled ?? [])];
  if (!enabled.includes(selected)) enabled.push(selected);

  return {
    ...current,
    ...gloss,
    scheme: inputScheme,
    last_chinese_scheme: inputScheme,
    ...(scheme === "xiaohe" ? { shuangpin_profile: shuangpinProfile } : {}),
    touch_keyboard_layout: scheme === "nine_key" ? "nine_key" : "twenty_six_key",
    touch_keyboard_schemes: {
      ...current.touch_keyboard_schemes,
      enabled,
      selected,
    },
  };
}
