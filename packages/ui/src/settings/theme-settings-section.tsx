import type { ReactNode } from "react";
import { GroupList, Row } from "../core/platform-controls";
import { ScreenKeyboardThemeSection } from "./screen-keyboard-theme-section";
import { SurfaceThemeSelect, type SurfaceTheme } from "./surface-theme-select";

export type ThemeMode = "dark" | "light" | "system";
export type { SurfaceTheme } from "./surface-theme-select";

/** The surfaces that can hold their own light or dark over the colour mode. The colour mode itself (`theme`) is the 明暗 group's segmented control on the 主题 page, not one of these. */
export type ThemePreferenceKey =
  | "settings_theme"
  | "screen_keyboard_theme"
  | "candidate_theme"
  | "toolbar_theme"
  | "menu_theme"
  | "emoji_theme"
  | "handwriting_theme"
  | "voice_theme";

export type ThemePreferences = Partial<Record<ThemePreferenceKey, SurfaceTheme>>;

export interface ThemeSettingsSectionProps {
  preferences: ThemePreferences;
  mobile: boolean;
  linux: boolean;
  floatingToolbar: boolean;
  desktopPanels: boolean;
  onChange: (key: ThemePreferenceKey, value: SurfaceTheme) => void;
}

/** One of the per-surface light/dark overrides in 高级. */
function SurfaceThemeRow({
  title,
  description,
  value,
  onChange,
}: {
  title: string;
  description: ReactNode;
  value: SurfaceTheme | undefined;
  onChange: (value: SurfaceTheme) => void;
}) {
  return (
    <Row title={title} description={description}>
      <SurfaceThemeSelect label={title} value={value ?? "follow"} onChange={onChange} />
    </Row>
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
      <SurfaceThemeRow
        title={mobile ? "候选栏主题" : "候选窗口主题"}
        description={
          mobile
            ? "覆盖候选栏的明暗外观；跟随时使用键盘主题"
            : linux
              ? "预览跟随颜色模式；IBus 候选窗口与 Fcitx5 经典界面按此明暗着色"
              : "预览跟随颜色模式"
        }
        value={preferences.candidate_theme}
        onChange={(value) => onChange("candidate_theme", value)}
      />
      {/* The Linux toolbar is the same desktop-drawn IBus property menu and Fcitx5 status menu, so no Linux host reads toolbar_theme. */}
      {floatingToolbar && !linux && (
        <SurfaceThemeRow
          title="悬浮工具栏主题"
          description="覆盖颜色模式；当前影响工具栏设置预览，原生工具栏需宿主支持"
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
