import {
  candidateFontSize,
  candidateFontSizes,
  candidatePreeditFontSize,
} from "../candidate/candidate-font-size";
import { SelectRow } from "./select-row";
import { SliderRow } from "./slider-row";

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

/** 设置宿主共用的候选字号控件：「候选窗口」页「字体与大小」组里的字号各行，标题不再写出界面名称，因为页面已经点明。 */
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

/** HarmonyOS 手机候选字号滑块提供的范围，即设计稿的 14–24 px；存储的偏好可以是 12 到 32 之间的任意字号。 */
export const candidateFontSizeSlider = { min: 14, max: 24, step: 1 } as const;

export interface CandidateFontSizeSliderRowProps {
  preferences: Pick<CandidateSizingPreferences, "candidate_font_size">;
  onChange: (patch: Pick<CandidateSizingPreferences, "candidate_font_size">) => void;
}

/** 「候选字号」滑块，HarmonyOS 手机「候选栏」页「候选栏」组的一行。超出滑块范围的存储字号显示在最近的一端，只有用户拖动滑块后才写回，所以打开页面不会改写偏好。 */
export function CandidateFontSizeSliderRow({
  preferences,
  onChange,
}: CandidateFontSizeSliderRowProps) {
  const size = Math.min(
    Math.max(candidateFontSize(preferences.candidate_font_size), candidateFontSizeSlider.min),
    candidateFontSizeSlider.max,
  );
  return (
    <SliderRow
      title="候选字号"
      {...candidateFontSizeSlider}
      value={size}
      valueText={`${size}px`}
      displayValue={`${size}px`}
      onChange={(candidate_font_size) => onChange({ candidate_font_size })}
    />
  );
}
