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
  mobile: boolean;
  showFontControls: boolean;
  showPreeditFont: boolean;
  onChange: (patch: Partial<CandidateSizingPreferences>) => void;
}

/** Shared candidate font-size controls for settings hosts: the size rows of the 候选窗口 page's 字体 group. */
export function CandidateSizingSection({
  preferences,
  mobile,
  showFontControls,
  showPreeditFont,
  onChange,
}: CandidateSizingSectionProps) {
  const surface = mobile ? "候选栏" : "候选窗";
  return (
    <>
      {showFontControls && (
        <SelectRow
          title={`${surface}字号`}
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
          title={`${surface}预编辑字号`}
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
