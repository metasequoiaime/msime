import { SkinToolbarPreview } from "../skin/skin-toolbar-preview";
import { candidateSkinPalette, type PreviewTheme } from "../skin/skin-preview-palette";
import type { FloatingToolbarPreferences } from "../index";
import * as settings from "./settings-style";

export interface FloatingToolbarToggleSectionProps {
  preferences: FloatingToolbarPreferences;
  skin: string;
  theme: PreviewTheme;
  onEnabledChange: (enabled: boolean) => void;
}

/** Controls the floating toolbar and keeps its skin-aware preview beside the switch. */
export function FloatingToolbarToggleSection({
  preferences,
  skin,
  theme,
  onEnabledChange,
}: FloatingToolbarToggleSectionProps) {
  return (
    <div className={`section ${settings.toolbarCard}`}>
      <label className={`section-header ${settings.toolbarSettingRow}`}>
        <span className="section-title">
          在桌面显示悬浮工具栏<small>快速访问输入法状态与常用功能</small>
        </span>
        <input
          aria-label="在桌面显示悬浮工具栏"
          className="toggle"
          type="checkbox"
          checked={preferences.enabled}
          onChange={(event) => onEnabledChange(event.target.checked)}
        />
      </label>
      <div className={settings.toolbarPreviewArea} aria-label="悬浮工具栏预览">
        <div className={settings.toolbarPreviewLabel}>预览</div>
        <div
          className={`${settings.skinCardPreview} skin-${skin}`}
          data-skin-preview=""
          data-toolbar-preview=""
          data-preview-theme={theme}
          style={candidateSkinPalette(skin, theme)}
        >
          <SkinToolbarPreview preferences={preferences} />
        </div>
      </div>
    </div>
  );
}
