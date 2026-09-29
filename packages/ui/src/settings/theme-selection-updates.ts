import type { TouchKeyboardSkinDesign } from "../keyboard/touch-keyboard-skin-design";
import { customThemeBase, type CustomCandidateColors } from "../theme/global-theme";
import type { Preferences } from "../index";

/** Applies one candidate colour while preserving the active theme's base and clearing its package. */
export function updateCandidateColor(
  current: Preferences,
  slot: keyof CustomCandidateColors,
  value: string | null,
): Preferences {
  const colors = { ...current.custom_theme?.candidate_colors, [slot]: value };
  const selecting = value !== null && (current.global_theme ?? "system") !== "custom";
  return {
    ...current,
    ...(value === null ? {} : { global_theme: "custom" as const }),
    custom_theme: selecting
      ? {
          ...current.custom_theme,
          base: customThemeBase(current.global_theme ?? "system", current.custom_theme),
          candidate_skin: null,
          candidate_colors: colors,
        }
      : { ...current.custom_theme, candidate_colors: colors },
  };
}

/** Selects the custom keyboard design, preserving the active theme's base when needed. */
export function updateCustomKeyboard(
  current: Preferences,
  design: TouchKeyboardSkinDesign,
): Preferences {
  const selecting = (current.global_theme ?? "system") !== "custom";
  return {
    ...current,
    global_theme: "custom",
    custom_theme: selecting
      ? {
          ...current.custom_theme,
          base: customThemeBase(current.global_theme ?? "system", current.custom_theme),
          candidate_skin: null,
          keyboard: design,
        }
      : { ...current.custom_theme, keyboard: design },
  };
}
