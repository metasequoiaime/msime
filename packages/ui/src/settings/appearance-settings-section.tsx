import type {
  HostCapabilities,
  MobileKeyboardFeedback,
  Preferences,
  SettingsClient,
} from "../index";
import { AppearanceCandidatePreview } from "../candidate/appearance-candidate-preview";
import { CandidateColorsSection } from "./candidate-colors-section";
import { CandidateFollowCursorSection } from "./candidate-follow-cursor-section";
import { CandidateFontControls } from "../candidate/candidate-font-controls";
import { CandidateFontUnsupportedNotice } from "./candidate-font-unsupported-notice";
import { CandidateLayoutSection } from "./candidate-layout-section";
import { CandidatePageSizeSection } from "./candidate-page-size-section";
import { CandidatePanelLimitSection } from "./candidate-panel-limit-section";
import { CandidatePaletteFallbackNotice } from "./candidate-palette-fallback-notice";
import { CandidateSizingSection } from "./candidate-sizing-section";
import { PreeditSettingsSection } from "./preedit-settings-section";
import { ThemeSettingsSection } from "./theme-settings-section";
import type { PreviewTheme } from "../skin/skin-preview-palette";

export interface AppearanceSettingsSectionProps {
  disabled: boolean;
  hidden: boolean;
  preferences: Preferences;
  revision: number;
  mobile: boolean;
  linux: boolean;
  windows: boolean;
  candidatePreviewTheme: PreviewTheme;
  candidatePanelLimit?: HostCapabilities["candidate_panel_limit"];
  showCandidateFollowCursor: boolean;
  showCandidateFontControls: boolean;
  showCandidateEnglishFont: boolean;
  showCandidatePreeditFont: boolean;
  showCandidateRowColors: boolean;
  showCandidateSelectionAppearance: boolean;
  showCandidateBorderColor: boolean;
  showShuangpinPreedit: boolean;
  fixedCandidatePageSize: boolean;
  fixedCandidateLayout: boolean;
  floatingToolbar: boolean;
  desktopPanels: boolean;
  candidatePaletteFollowsDesktop?: boolean;
  inlinePreedit?: boolean;
  inlinePreeditBusy: boolean;
  scan?: SettingsClient["scanSkinCatalog"];
  readImage?: SettingsClient["readSkinImage"];
  resolveFonts?: SettingsClient["resolveFontFamilies"];
  listFontFamilies?: SettingsClient["listFontFamilies"];
  onPreferencesChange: (patch: Partial<Preferences>) => void;
  onInlinePreeditChange: (value: boolean) => void;
}

/** Composes candidate appearance settings shared by desktop and touch hosts. */
export function AppearanceSettingsSection({
  disabled,
  hidden,
  preferences,
  revision,
  mobile,
  linux,
  windows,
  candidatePreviewTheme,
  candidatePanelLimit,
  showCandidateFollowCursor,
  showCandidateFontControls,
  showCandidateEnglishFont,
  showCandidatePreeditFont,
  showCandidateRowColors,
  showCandidateSelectionAppearance,
  showCandidateBorderColor,
  showShuangpinPreedit,
  fixedCandidatePageSize,
  fixedCandidateLayout,
  floatingToolbar,
  desktopPanels,
  candidatePaletteFollowsDesktop,
  inlinePreedit,
  inlinePreeditBusy,
  scan,
  readImage,
  resolveFonts,
  listFontFamilies,
  onPreferencesChange,
  onInlinePreeditChange,
}: AppearanceSettingsSectionProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="外观">
      <AppearanceCandidatePreview
        preferences={{
          ...preferences,
          candidate_english_font: windows
            ? (preferences.candidate_english_font ?? "Segoe UI")
            : preferences.candidate_english_font,
        }}
        scan={scan}
        readImage={readImage}
        resolveFonts={resolveFonts}
        active={!hidden}
        revision={revision}
        mobile={mobile}
      />
      {candidatePanelLimit && <CandidatePanelLimitSection limit={candidatePanelLimit} />}
      {showCandidateFollowCursor && (
        <CandidateFollowCursorSection
          value={preferences.candidate_follow_cursor}
          onChange={(candidate_follow_cursor) => onPreferencesChange({ candidate_follow_cursor })}
        />
      )}
      {showCandidateFontControls ? (
        <CandidateFontControls
          value={preferences}
          onChange={onPreferencesChange}
          readFonts={listFontFamilies}
          windows={windows}
          englishFont={showCandidateEnglishFont}
          mobile={mobile}
        />
      ) : (
        <CandidateFontUnsupportedNotice />
      )}
      <CandidateSizingSection
        preferences={preferences}
        mobile={mobile}
        showFontControls={showCandidateFontControls}
        showPreeditFont={showCandidatePreeditFont}
        onChange={onPreferencesChange}
      />
      {candidatePaletteFollowsDesktop === false && <CandidatePaletteFallbackNotice />}
      <CandidateColorsSection
        preferences={preferences}
        previewTheme={candidatePreviewTheme}
        showRowColors={showCandidateRowColors}
        showSelectionAppearance={showCandidateSelectionAppearance}
        showBorderColor={showCandidateBorderColor}
        linux={linux}
        onChange={(key, value) => onPreferencesChange({ [key]: value })}
      />
      <CandidatePageSizeSection
        value={preferences.candidate_page_size}
        fixed={fixedCandidatePageSize}
        onChange={(candidate_page_size) => onPreferencesChange({ candidate_page_size })}
      />
      <ThemeSettingsSection
        preferences={preferences}
        mobile={mobile}
        linux={linux}
        floatingToolbar={floatingToolbar}
        desktopPanels={desktopPanels}
        onChange={(key, value) => onPreferencesChange({ [key]: value })}
      />
      <CandidateLayoutSection
        value={preferences.candidate_layout}
        fixed={fixedCandidateLayout}
        onChange={(candidate_layout) => onPreferencesChange({ candidate_layout })}
      />
      <PreeditSettingsSection
        preferences={preferences}
        mobile={mobile}
        showShuangpinPreedit={showShuangpinPreedit}
        inlinePreedit={inlinePreedit}
        inlinePreeditBusy={inlinePreeditBusy}
        onChange={onPreferencesChange}
        onInlinePreeditChange={onInlinePreeditChange}
      />
    </fieldset>
  );
}
