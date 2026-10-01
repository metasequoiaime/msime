import {
  candidateFontSize,
  candidateFontSizes,
  candidatePreeditFontSize,
} from "../candidate/candidate-font-size";
import { SelectRow } from "./select-row";

export interface CandidateSizingPreferences {
  candidate_font_size?: number;
  candidate_preedit_font_size?: number;
}

export interface CandidateSizingSectionProps {
  preferences: CandidateSizingPreferences;
  showFontControls: boolean;
  showPreeditFont: boolean;
  onChange: (patch: Partial<CandidateSizingPreferences>) => void;
}

/** Shared candidate font-size controls for settings hosts: the size rows of the 候选窗口 page's 字体与大小 group, titled without the surface since the page already names it. */
export function CandidateSizingSection({
  preferences,
  showFontControls,
  showPreeditFont,
  onChange,
}: CandidateSizingSectionProps) {
  return (
    <>
      {showFontControls && (
        <SelectRow
          title={"字号"}
          value={candidateFontSize(preferences.candidate_font_size)}
          onChange={(event) => onChange({ candidate_font_size: Number(event.target.value) })}
        >
          {candidateFontSizes.map((size) => (
            <option key={size} value={size}>
              {size}
            </option>
          ))}
        </SelectRow>
      )}
      {showPreeditFont && (
        <SelectRow
          title={"预编辑字号"}
          value={candidatePreeditFontSize(preferences.candidate_preedit_font_size)}
          onChange={(event) =>
            onChange({ candidate_preedit_font_size: Number(event.target.value) })
          }
        >
          {candidateFontSizes.map((size) => (
            <option key={size} value={size}>
              {size}
            </option>
          ))}
        </SelectRow>
      )}
    </>
  );
}
