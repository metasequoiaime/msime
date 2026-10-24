import type { HostPlatform } from "../core/host-capabilities";
import type { Preferences } from "../core/preferences-model";
import type {
  CandidateFontPreferences,
  CandidateSizingPreferences,
} from "../candidate/candidate-preferences";
import { currentCandidateFontPreset } from "../candidate/candidate-font-presets";
import { CandidateFontControls } from "../candidate/candidate-font-controls";
import type { FontCatalogReader } from "../candidate/font-catalog";
import { CandidateSizingSection } from "./candidate-sizing-section";
import { CandidateFontPresetRow } from "./candidate-window-style-section";

export type CandidateFontSettingsRowsPreferences = CandidateFontPreferences &
  CandidateSizingPreferences;

export interface CandidateFontSettingsRowsProps {
  preferences: CandidateFontSettingsRowsPreferences;
  platform?: HostPlatform;
  readFonts?: FontCatalogReader;
  showFontSize: boolean;
  showPreeditFont: boolean;
  englishFont: boolean;
  custom?: boolean;
  onCustomChange?: (custom: boolean) => void;
  onChange: (patch: Partial<Preferences>) => void;
}

/** 候选窗口页共用的字体预设、字体输入和字号行；手机不提供「自定义…」，字体框始终显示。 */
export function CandidateFontSettingsRows({
  preferences,
  platform,
  readFonts,
  showFontSize,
  showPreeditFont,
  englishFont,
  custom = false,
  onCustomChange,
  onChange,
}: CandidateFontSettingsRowsProps) {
  const showMainFont =
    !onCustomChange || custom || currentCandidateFontPreset(preferences, platform) === null;
  return (
    <>
      <CandidateFontPresetRow
        preferences={preferences}
        platform={platform}
        onChange={onChange}
        custom={custom}
        onCustomChange={onCustomChange}
      />
      {(showMainFont || englishFont) && (
        <CandidateFontControls
          value={preferences}
          onChange={onChange}
          readFonts={readFonts}
          windows={platform === "windows"}
          mainFont={showMainFont}
          englishFont={englishFont}
        />
      )}
      <CandidateSizingSection
        preferences={preferences}
        showFontControls={showFontSize}
        showPreeditFont={showPreeditFont}
        showTranslationFont={platform === "macos"}
        onChange={onChange}
      />
    </>
  );
}
