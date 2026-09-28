import type { PointerEvent as ReactPointerEvent } from "react";
import type { CommunitySkinClient } from "../community/community-skins";
import { TouchKeyboardSkinEditor } from "../keyboard/touch-keyboard-skin-editor";
import type {
  AiSkinClient,
  CustomSkinLibraryClient,
  TouchKeyboardSkinDesign,
} from "../keyboard/touch-keyboard-skin-design";
import type { TouchKeyboardSkin } from "../keyboard/screen-keyboard-preview";
import type { PreviewTheme } from "../skin/skin-preview-palette";
import type { SurfaceTheme } from "./theme-settings-section";
import { ScreenKeyboardCommunitySection } from "./screen-keyboard-community-section";
import { ScreenKeyboardLaunchSection } from "./screen-keyboard-launch-section";
import { ScreenKeyboardSkinsSection } from "./screen-keyboard-skins-section";
import { ScreenKeyboardThemeSection } from "./screen-keyboard-theme-section";
import {
  TouchKeyboardGeometrySection,
  type TouchToolbarPreferences,
} from "./touch-keyboard-geometry-section";

export interface ScreenKeyboardSettingsSectionProps {
  disabled: boolean;
  hidden: boolean;
  mobile: boolean;
  screenKeyboardTheme: SurfaceTheme;
  previewTheme: PreviewTheme;
  onScreenKeyboardThemeChange: (value: SurfaceTheme) => void;
  selectedSkin: TouchKeyboardSkin;
  customDesign: TouchKeyboardSkinDesign;
  customAvailable: boolean;
  editorOpen: boolean;
  onSkinSelect: (skin: TouchKeyboardSkin) => void;
  onToggleEditor: () => void;
  communityAvailable: boolean;
  onOpenCommunity: () => void;
  library?: CustomSkinLibraryClient;
  aiSkins?: AiSkinClient;
  communitySkins?: CommunitySkinClient;
  onDesignChange: (design: TouchKeyboardSkinDesign) => void;
  onUseDesign: () => void;
  onCloseEditor: () => void;
  heightAdjustment: number;
  keySpacingTenths: number;
  rowSpacingTenths: number;
  touchVoiceShortcut: boolean;
  toolbarComponents: boolean;
  toolbar?: Partial<TouchToolbarPreferences>;
  tabletFullKeys?: boolean;
  tabletFullKeysBusy: boolean;
  onHeightAdjustmentChange: (value: number) => void;
  onKeySpacingChange: (value: number) => void;
  onRowSpacingChange: (value: number) => void;
  onTouchVoiceShortcutChange: (enabled: boolean) => void;
  onToolbarChange: (value: TouchToolbarPreferences) => void;
  onTabletFullKeysChange: (enabled: boolean) => void;
  onReset: () => void;
  openScreenKeyboard?: () => void | Promise<void>;
  onPointerDown: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onPointerMove: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onPointerUp: (event: ReactPointerEvent<HTMLDivElement>) => void;
  onPointerCancel: (event: ReactPointerEvent<HTMLDivElement>) => void;
}

/** Composes the screen keyboard settings page shared by touch hosts. */
export function ScreenKeyboardSettingsSection({
  disabled,
  hidden,
  mobile,
  screenKeyboardTheme,
  previewTheme,
  onScreenKeyboardThemeChange,
  selectedSkin,
  customDesign,
  customAvailable,
  editorOpen,
  onSkinSelect,
  onToggleEditor,
  communityAvailable,
  onOpenCommunity,
  library,
  aiSkins,
  communitySkins,
  onDesignChange,
  onUseDesign,
  onCloseEditor,
  heightAdjustment,
  keySpacingTenths,
  rowSpacingTenths,
  touchVoiceShortcut,
  toolbarComponents,
  toolbar,
  tabletFullKeys,
  tabletFullKeysBusy,
  onHeightAdjustmentChange,
  onKeySpacingChange,
  onRowSpacingChange,
  onTouchVoiceShortcutChange,
  onToolbarChange,
  onTabletFullKeysChange,
  onReset,
  openScreenKeyboard,
  onPointerDown,
  onPointerMove,
  onPointerUp,
  onPointerCancel,
}: ScreenKeyboardSettingsSectionProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="屏幕键盘">
      <ScreenKeyboardThemeSection
        mobile={mobile}
        value={screenKeyboardTheme}
        onChange={onScreenKeyboardThemeChange}
      />
      <ScreenKeyboardSkinsSection
        mobile={mobile}
        theme={previewTheme}
        selected={selectedSkin}
        customDesign={customDesign}
        customAvailable={customAvailable}
        editorOpen={editorOpen}
        onSelect={onSkinSelect}
        onToggleEditor={onToggleEditor}
      />
      {communityAvailable && <ScreenKeyboardCommunitySection onOpen={onOpenCommunity} />}
      {customAvailable && editorOpen && (
        <div className="section">
          <TouchKeyboardSkinEditor
            design={customDesign}
            selected={selectedSkin === "custom"}
            theme={previewTheme}
            disabled={disabled}
            library={library}
            aiSkins={aiSkins}
            communitySkins={communitySkins}
            onChange={onDesignChange}
            onUse={onUseDesign}
            onClose={onCloseEditor}
          />
        </div>
      )}
      <TouchKeyboardGeometrySection
        heightAdjustment={heightAdjustment}
        keySpacingTenths={keySpacingTenths}
        rowSpacingTenths={rowSpacingTenths}
        touchVoiceShortcut={touchVoiceShortcut}
        toolbarComponents={toolbarComponents}
        toolbar={toolbar}
        tabletFullKeys={tabletFullKeys}
        tabletFullKeysBusy={tabletFullKeysBusy}
        onHeightAdjustmentChange={onHeightAdjustmentChange}
        onKeySpacingChange={onKeySpacingChange}
        onRowSpacingChange={onRowSpacingChange}
        onTouchVoiceShortcutChange={onTouchVoiceShortcutChange}
        onToolbarChange={onToolbarChange}
        onTabletFullKeysChange={onTabletFullKeysChange}
        onReset={onReset}
      />
      <ScreenKeyboardLaunchSection
        openScreenKeyboard={openScreenKeyboard}
        theme={previewTheme}
        skin={selectedSkin}
        customDesign={customDesign}
        keySpacingTenths={keySpacingTenths}
        rowSpacingTenths={rowSpacingTenths}
        heightAdjustment={heightAdjustment}
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerCancel}
      />
    </fieldset>
  );
}
