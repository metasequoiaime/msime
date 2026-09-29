import type { MobileKeyboardFeedback } from "./mobile-keyboard-feedback-section";
import type { SettingsVisualPagesProps } from "./settings-visual-pages";
import type { SettingsPageId } from "./mobile-navigation";
import type {
  FloatingToolbarPreferences,
  HostCapabilities,
  Preferences,
  SettingsClient,
  Snapshot,
} from "../index";

type AppearanceProps = SettingsVisualPagesProps["appearance"];
type SkinProps = SettingsVisualPagesProps["skin"];
type FloatingToolbarProps = SettingsVisualPagesProps["floatingToolbar"];

export interface SettingsPageVisualModelOptions {
  page: SettingsPageId;
  draft: Preferences;
  busy: boolean;
  snapshot?: Snapshot;
  host?: HostCapabilities;
  mobilePlatform: boolean;
  linuxPlatform: boolean;
  windowsPlatform: boolean;
  showFloatingToolbar: boolean;
  showToolbarAppearance: boolean;
  showToolbarComponents: boolean;
  desktopPanels: boolean;
  showCandidateFollowCursor: boolean;
  showCandidateFontControls: boolean;
  showCandidateEnglishFont: boolean;
  showCandidatePreeditFont: boolean;
  showCandidateRowColors: boolean;
  showCandidateSelectionAppearance: boolean;
  showCandidateBorderColor: boolean;
  showShuangpinPreedit: boolean;
  mobileKeyboardFeedback?: MobileKeyboardFeedback;
  mobileKeyboardFeedbackBusy: boolean;
  candidatePreviewTheme: AppearanceProps["candidatePreviewTheme"];
  toolbarPreviewTheme: FloatingToolbarProps["theme"];
  floatingToolbar: FloatingToolbarPreferences;
  skinPreviewThemes: SkinProps["previewThemes"];
  client: SettingsClient;
  appearanceSettingsActions: Pick<AppearanceProps, "onPreferencesChange" | "onInlinePreeditChange">;
  skinSettingsActions: Pick<SkinProps, "onCandidatePaletteChange" | "onSelect" | "onTogglePreview">;
  floatingToolbarActions: Pick<
    FloatingToolbarProps,
    "onEnabledChange" | "onScaleChange" | "onFontSizeChange" | "onComponentChange"
  >;
}

/** Builds the visual settings props from the page controller's state and actions. */
export function settingsPageVisualModel({
  page,
  draft,
  busy,
  snapshot,
  host,
  mobilePlatform,
  linuxPlatform,
  windowsPlatform,
  showFloatingToolbar,
  showToolbarAppearance,
  showToolbarComponents,
  desktopPanels,
  showCandidateFollowCursor,
  showCandidateFontControls,
  showCandidateEnglishFont,
  showCandidatePreeditFont,
  showCandidateRowColors,
  showCandidateSelectionAppearance,
  showCandidateBorderColor,
  showShuangpinPreedit,
  mobileKeyboardFeedback,
  mobileKeyboardFeedbackBusy,
  candidatePreviewTheme,
  toolbarPreviewTheme,
  floatingToolbar,
  skinPreviewThemes,
  client,
  appearanceSettingsActions,
  skinSettingsActions,
  floatingToolbarActions,
}: SettingsPageVisualModelOptions): SettingsVisualPagesProps {
  return {
    appearance: {
      disabled: busy,
      hidden: page !== "appearance",
      preferences: draft,
      revision: snapshot?.revision ?? 0,
      mobile: mobilePlatform,
      linux: linuxPlatform,
      windows: host?.platform === "windows",
      candidatePreviewTheme,
      candidatePanelLimit: host?.candidate_panel_limit,
      showCandidateFollowCursor,
      showCandidateFontControls,
      showCandidateEnglishFont,
      showCandidatePreeditFont,
      showCandidateRowColors,
      showCandidateSelectionAppearance,
      showCandidateBorderColor,
      showShuangpinPreedit,
      fixedCandidatePageSize: host?.fixed_candidate_page_size !== undefined,
      fixedCandidateLayout: host?.fixed_candidate_layout !== undefined,
      floatingToolbar: showFloatingToolbar,
      desktopPanels,
      candidatePaletteFollowsDesktop: mobileKeyboardFeedback?.candidatePaletteFollowsDesktop,
      inlinePreedit: mobileKeyboardFeedback?.inlinePreedit,
      inlinePreeditBusy: mobileKeyboardFeedbackBusy,
      scan: client.scanSkinCatalog,
      readImage: client.readSkinImage,
      resolveFonts: client.resolveFontFamilies,
      listFontFamilies: client.listFontFamilies,
      ...appearanceSettingsActions,
    },
    skin: {
      disabled: busy,
      hidden: page !== "skin",
      mobile: mobilePlatform,
      linux: linuxPlatform,
      candidatePanelLimit: host?.candidate_panel_limit,
      mobileKeyboardFeedback,
      mobileKeyboardFeedbackBusy,
      ...skinSettingsActions,
      candidateSkinCatalog: snapshot?.candidate_skin_catalog,
      selected: draft.candidate_skin ?? "willow_green",
      previewThemes: skinPreviewThemes,
      defaultTheme: candidatePreviewTheme,
      activeTheme: candidatePreviewTheme,
      scan: client.scanSkinCatalog,
      openDirectory: client.openSkinDirectory,
      importsSkin: host?.skin_directory_import === true,
      readImage: client.readSkinImage,
      readFont: client.readSkinFont,
      readToolbarCss: client.readSkinToolbarCss,
      layout: host?.fixed_candidate_layout ?? draft.candidate_layout ?? "vertical",
      toolbarPreview: !linuxPlatform,
    },
    floatingToolbar: {
      disabled: busy,
      hidden: page !== "floating-toolbar",
      preferences: floatingToolbar,
      skin: draft.candidate_skin ?? "willow_green",
      theme: toolbarPreviewTheme,
      ...floatingToolbarActions,
      showAppearance: showToolbarAppearance,
      showComponents: showToolbarComponents,
      capabilities: host
        ? {
            floating_toolbar_handwriting: host.floating_toolbar_handwriting,
            floating_toolbar_voice: host.floating_toolbar_voice,
          }
        : undefined,
    },
  };
}
