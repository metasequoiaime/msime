import {
  candidateFontSize,
  candidateFontSizes,
  candidatePreeditFontSize,
} from "../candidate/candidate-font-size";
import { Row, Select } from "../core/platform-controls";

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
        <Row title={`${surface}字号`}>
          <Select
            value={candidateFontSize(preferences.candidate_font_size)}
            onChange={(event) => onChange({ candidate_font_size: Number(event.target.value) })}
          >
            {candidateFontSizes.map((size) => (
              <option key={size} value={size}>
                {size}
              </option>
            ))}
          </Select>
        </Row>
      )}
      {showPreeditFont && (
        <Row title={`${surface}预编辑字号`}>
          <Select
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
          </Select>
        </Row>
      )}
    </>
  );
}
