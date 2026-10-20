import { useMacOSLiquidGlass } from "../core/use-macos-liquid-glass";
import { GroupList } from "../core/platform-controls";
import { ScreenKeyboardThemeSection } from "./screen-keyboard-theme-section";
import { SurfaceThemeRow } from "./surface-theme-row";
import type { SurfaceTheme, ThemePreferenceKey, ThemePreferences } from "./theme-preferences";

export type {
  SurfaceTheme,
  ThemeMode,
  ThemePreferenceKey,
  ThemePreferences,
} from "./theme-preferences";

export interface ThemeSettingsSectionProps {
  preferences: ThemePreferences;
  mobile: boolean;
  linux: boolean;
  floatingToolbar: boolean;
  desktopPanels: boolean;
  onChange: (key: ThemePreferenceKey, value: SurfaceTheme) => void;
}

/** 「候选窗口主题」一行：候选窗口在颜色模式之上单独设明暗。各界面一律跟随颜色模式的宿主（`surface_themes_follow`）只剩这一项，主题页把它放进「明暗」组；其他宿主仍在「高级」组里与各界面并列。 */
export function CandidateThemeRow({
  value,
  mobile,
  linux,
  onChange,
}: {
  value: SurfaceTheme | undefined;
  mobile: boolean;
  linux: boolean;
  onChange: (value: SurfaceTheme) => void;
}) {
  const glass = useMacOSLiquidGlass();
  return (
    <SurfaceThemeRow
      title={mobile ? "候选栏主题" : "候选窗口主题"}
      description={
        mobile
          ? "覆盖候选栏的明暗外观；跟随时使用键盘主题"
          : linux
            ? "覆盖颜色模式；IBus 候选窗口与 Fcitx5 经典界面按此明暗着色"
            : glass
              ? "系统主题自动参考输入区配色，不支持时跟随系统；可单独指定明暗"
              : "覆盖颜色模式，只影响候选窗口"
      }
      value={value}
      onChange={onChange}
    />
  );
}

/** The 主题 page's 高级 group, shared by desktop and touch settings hosts: each surface can still hold its own light or dark over the colour mode. */
export function ThemeSettingsSection({
  preferences,
  mobile,
  linux,
  floatingToolbar,
  desktopPanels,
  onChange,
}: ThemeSettingsSectionProps) {
  return (
    <GroupList title="高级">
      <SurfaceThemeRow
        title="设置界面主题"
        description="覆盖颜色模式，仅影响当前设置窗口"
        value={preferences.settings_theme}
        onChange={(value) => onChange("settings_theme", value)}
      />
      <ScreenKeyboardThemeSection
        mobile={mobile}
        value={preferences.screen_keyboard_theme ?? "follow"}
        onChange={(value) => onChange("screen_keyboard_theme", value)}
      />
      <CandidateThemeRow
        mobile={mobile}
        linux={linux}
        value={preferences.candidate_theme}
        onChange={(value) => onChange("candidate_theme", value)}
      />
      {/* Linux 宿主都不读 toolbar_theme：X11 上的悬浮工具栏跟随颜色模式，与切换徽章相同；「工具栏」子菜单由桌面面板按它自己的主题画。 */}
      {floatingToolbar && !linux && (
        <SurfaceThemeRow
          title="悬浮工具栏主题"
          description="覆盖颜色模式，只影响悬浮工具栏"
          value={preferences.toolbar_theme}
          onChange={(value) => onChange("toolbar_theme", value)}
        />
      )}
      {/* Linux menus are the IBus property menu and the Fcitx5 status menu, drawn by the desktop panel in its own theme. */}
      {!mobile && !linux && (
        <SurfaceThemeRow
          title="菜单主题"
          description="覆盖托盘菜单与候选右键菜单的明暗外观"
          value={preferences.menu_theme}
          onChange={(value) => onChange("menu_theme", value)}
        />
      )}
      <SurfaceThemeRow
        title="表情面板主题"
        description="覆盖 Emoji、颜文字和符号面板的明暗外观"
        value={preferences.emoji_theme}
        onChange={(value) => onChange("emoji_theme", value)}
      />
      <SurfaceThemeRow
        title="手写识别板主题"
        description="覆盖手写识别板的明暗外观"
        value={preferences.handwriting_theme}
        onChange={(value) => onChange("handwriting_theme", value)}
      />
      {desktopPanels && (
        <SurfaceThemeRow
          title="语音输入弹出条主题"
          description="覆盖语音输入面板的明暗外观"
          value={preferences.voice_theme}
          onChange={(value) => onChange("voice_theme", value)}
        />
      )}
    </GroupList>
  );
}
