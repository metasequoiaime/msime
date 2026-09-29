import type {
  ExternalSkinCatalog,
  HostCapabilities,
  MobileKeyboardFeedback,
  Preferences,
  SettingsClient,
} from "../index";
import { ExternalSkins } from "../skin/external-skins";
import type { PreviewTheme } from "../skin/skin-preview-palette";
import * as settings from "./settings-style";
import { BuiltInSkinsSection } from "./built-in-skins-section";
import { CandidatePaletteSection } from "./candidate-palette-section";
import { CandidatePanelLimitSection } from "./candidate-panel-limit-section";
import { SkinPlatformNotice } from "./skin-platform-notice";

export interface SkinSettingsSectionProps {
  disabled: boolean;
  hidden: boolean;
  mobile: boolean;
  linux: boolean;
  candidatePanelLimit?: HostCapabilities["candidate_panel_limit"];
  mobileKeyboardFeedback?: MobileKeyboardFeedback;
  mobileKeyboardFeedbackBusy: boolean;
  onCandidatePaletteChange: (value: boolean) => void;
  candidateSkinCatalog?: ExternalSkinCatalog;
  selected: string;
  previewThemes: Partial<Record<NonNullable<Preferences["candidate_skin"]>, PreviewTheme>>;
  defaultTheme: PreviewTheme;
  onSelect: (id: string) => void;
  onTogglePreview: (id: string) => void;
  activeTheme: PreviewTheme;
  scan?: SettingsClient["scanSkinCatalog"];
  openDirectory?: SettingsClient["openSkinDirectory"];
  importsSkin?: boolean;
  readImage?: SettingsClient["readSkinImage"];
  readFont?: SettingsClient["readSkinFont"];
  readToolbarCss?: SettingsClient["readSkinToolbarCss"];
  layout: "horizontal" | "vertical";
  toolbarPreview: boolean;
}

/** Composes the candidate skin settings shared by desktop and mobile hosts. */
export function SkinSettingsSection({
  disabled,
  hidden,
  mobile,
  linux,
  candidatePanelLimit,
  mobileKeyboardFeedback,
  mobileKeyboardFeedbackBusy,
  onCandidatePaletteChange,
  candidateSkinCatalog,
  selected,
  previewThemes,
  defaultTheme,
  onSelect,
  onTogglePreview,
  activeTheme,
  scan,
  openDirectory,
  importsSkin,
  readImage,
  readFont,
  readToolbarCss,
  layout,
  toolbarPreview,
}: SkinSettingsSectionProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="皮肤">
      <SkinPlatformNotice mobile={mobile} linux={linux} />
      {candidatePanelLimit && <CandidatePanelLimitSection limit={candidatePanelLimit} />}
      {mobileKeyboardFeedback?.candidatePaletteFollowsDesktop !== undefined && (
        <CandidatePaletteSection
          value={mobileKeyboardFeedback.candidatePaletteFollowsDesktop}
          busy={mobileKeyboardFeedbackBusy}
          onChange={onCandidatePaletteChange}
        />
      )}
      {candidateSkinCatalog && (
        <div className={settings.externalMeta} role="status">
          外部皮肤目录：
          {candidateSkinCatalog.scanned
            ? `已扫描（${candidateSkinCatalog.packages.length} 个）`
            : "尚未扫描"}
          {candidateSkinCatalog.issues?.length
            ? `，${candidateSkinCatalog.issues.length} 个问题`
            : ""}
        </div>
      )}
      <BuiltInSkinsSection
        selected={selected}
        previewThemes={previewThemes}
        defaultTheme={defaultTheme}
        linux={linux}
        onSelect={onSelect}
        onTogglePreview={onTogglePreview}
      />
      <ExternalSkins
        activeTheme={activeTheme}
        scan={scan}
        openDirectory={openDirectory}
        importsSkin={importsSkin}
        readImage={readImage}
        readFont={readFont}
        readToolbarCss={readToolbarCss}
        selected={selected}
        layout={layout}
        onSelect={onSelect}
        toolbarPreview={toolbarPreview}
      />
    </fieldset>
  );
}
