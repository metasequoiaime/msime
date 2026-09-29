import type { FloatingToolbarPreferences } from "../index";
import {
  FloatingToolbarAppearanceSection,
  type FloatingToolbarFontSize,
  type FloatingToolbarScale,
} from "./floating-toolbar-appearance-section";
import {
  FloatingToolbarComponentsSection,
  type FloatingToolbarCapability,
  type FloatingToolbarComponentKey,
} from "./floating-toolbar-components-section";
import { FloatingToolbarPlatformNotice } from "./floating-toolbar-platform-notice";
import { FloatingToolbarToggleSection } from "./floating-toolbar-toggle-section";
import type { PreviewTheme } from "../skin/skin-preview-palette";

export interface FloatingToolbarSettingsSectionProps {
  disabled: boolean;
  hidden: boolean;
  preferences: FloatingToolbarPreferences;
  skin: string;
  theme: PreviewTheme;
  onEnabledChange: (enabled: boolean) => void;
  showAppearance: boolean;
  showComponents: boolean;
  capabilities?: Partial<Record<FloatingToolbarCapability, boolean>>;
  onScaleChange: (value: FloatingToolbarScale) => void;
  onFontSizeChange: (value: FloatingToolbarFontSize) => void;
  onComponentChange: (key: FloatingToolbarComponentKey, enabled: boolean) => void;
}

/** Floating toolbar settings page composition shared by desktop hosts. */
export function FloatingToolbarSettingsSection({
  disabled,
  hidden,
  preferences,
  skin,
  theme,
  onEnabledChange,
  showAppearance,
  showComponents,
  capabilities,
  onScaleChange,
  onFontSizeChange,
  onComponentChange,
}: FloatingToolbarSettingsSectionProps) {
  return (
    <fieldset disabled={disabled} hidden={hidden} aria-label="悬浮工具栏">
      <FloatingToolbarToggleSection
        preferences={preferences}
        skin={skin}
        theme={theme}
        onEnabledChange={onEnabledChange}
      />
      {!showAppearance && <FloatingToolbarPlatformNotice />}
      {showAppearance && (
        <FloatingToolbarAppearanceSection
          scale={preferences.scale_percent}
          fontSize={preferences.font_size}
          onScaleChange={onScaleChange}
          onFontSizeChange={onFontSizeChange}
        />
      )}
      {showComponents && (
        <FloatingToolbarComponentsSection
          values={preferences}
          capabilities={capabilities}
          onChange={onComponentChange}
        />
      )}
    </fieldset>
  );
}
