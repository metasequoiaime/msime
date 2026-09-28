import {
  candidateFontSize,
  candidateFontSizes,
  candidatePreeditFontSize,
} from "../candidate/candidate-font-size";

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

/** Shared candidate font-size controls for settings hosts. */
export function CandidateSizingSection({
  preferences,
  mobile,
  showFontControls,
  showPreeditFont,
  onChange,
}: CandidateSizingSectionProps) {
  return (
    <>
      {showFontControls && (
        <div className="section">
          <label className="section-header">
            <span className="section-title">{mobile ? "候选栏字号" : "候选窗字号"}</span>
            <select
              aria-label={mobile ? "候选栏字号" : "候选窗字号"}
              value={candidateFontSize(preferences.candidate_font_size)}
              onChange={(event) => onChange({ candidate_font_size: Number(event.target.value) })}
            >
              {candidateFontSizes.map((size) => (
                <option key={size} value={size}>
                  {size}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}
      {showPreeditFont && (
        <div className="section">
          <label className="section-header">
            <span className="section-title">
              {mobile ? "候选栏预编辑字号" : "候选窗预编辑字号"}
            </span>
            <select
              aria-label={mobile ? "候选栏预编辑字号" : "候选窗预编辑字号"}
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
            </select>
          </label>
        </div>
      )}
    </>
  );
}
