import type { CSSProperties } from "react";
import { candidateFamilyStyle, type CandidateFontPreferences } from "./candidate-font-family";
import { candidateFontStyle } from "./candidate-font-size";
import { candidateTextStyle } from "./candidate-text-color";

export type CandidateAppearancePreferences = CandidateFontPreferences & {
  candidate_font_size?: number;
  candidate_preedit_font_size?: number;
  candidate_text_color?: unknown;
  candidate_number_color?: unknown;
  candidate_accent_color?: unknown;
  candidate_selected_color?: unknown;
  candidate_hover_color?: unknown;
  candidate_surface_color?: unknown;
  candidate_border_color?: unknown;
};

export function candidateAppearanceStyle(
  preferences: CandidateAppearancePreferences,
): CSSProperties {
  return {
    ...candidateFontStyle(preferences),
    ...candidateTextStyle(
      preferences.candidate_text_color,
      preferences.candidate_number_color,
      preferences.candidate_accent_color,
      preferences.candidate_selected_color,
      preferences.candidate_hover_color,
      preferences.candidate_surface_color,
      preferences.candidate_border_color,
    ),
    ...candidateFamilyStyle(preferences),
  };
}
