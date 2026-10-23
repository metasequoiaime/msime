import { defaultFloatingToolbar } from "./floating-toolbar-defaults";

/** 悬浮工具栏设置页、预览和各宿主共同使用的偏好模型。 */
export type FloatingToolbarPreferences = {
  enabled: boolean;
  english_mode: boolean;
  /** 切换输入方案的按钮。 */
  input_scheme: boolean;
  fullwidth: boolean;
  punctuation: boolean;
  character_set: boolean;
  emoji: boolean;
  handwriting: boolean;
  screen_keyboard: boolean;
  voice: boolean;
  settings: boolean;
  /** 显示一段时间无交互后把工具栏淡出隐藏。出厂默认关，对齐 Windows 旧正式版 v0.11.1。 */
  auto_hide: boolean;
  /** 自动隐藏延时（秒），可选 1–60，出厂默认 5。 */
  auto_hide_delay: number;
  /** 工具栏周围的柔和阴影。出厂默认开，对齐 Windows 旧正式版 v0.11.1。 */
  shadow: boolean;
  scale_percent: 75 | 100 | 125 | 150;
  font_size: 16 | 18 | 20 | 22 | 24 | 26 | 28;
};

export type FloatingToolbarPreferencesSource = {
  floating_toolbar?: Partial<FloatingToolbarPreferences>;
};

/** 为设置页和预览投影有效的悬浮工具栏偏好。 */
export function floatingToolbarPreferences(
  draft?: FloatingToolbarPreferencesSource,
): FloatingToolbarPreferences {
  return { ...defaultFloatingToolbar, ...draft?.floating_toolbar };
}
