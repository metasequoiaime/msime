export type ThemeMode = "dark" | "light" | "system";
export type SurfaceTheme = "follow" | "dark" | "light";

export type ThemePreferenceKey =
  | "theme"
  | "settings_theme"
  | "candidate_theme"
  | "toolbar_theme"
  | "menu_theme"
  | "emoji_theme"
  | "handwriting_theme"
  | "voice_theme";

export interface ThemePreferences {
  theme?: ThemeMode;
  settings_theme?: SurfaceTheme;
  candidate_theme?: SurfaceTheme;
  toolbar_theme?: SurfaceTheme;
  menu_theme?: SurfaceTheme;
  emoji_theme?: SurfaceTheme;
  handwriting_theme?: SurfaceTheme;
  voice_theme?: SurfaceTheme;
}

export interface ThemeSettingsSectionProps {
  preferences: ThemePreferences;
  mobile: boolean;
  linux: boolean;
  floatingToolbar: boolean;
  desktopPanels: boolean;
  onChange: (key: ThemePreferenceKey, value: ThemeMode | SurfaceTheme) => void;
}

type ThemeSelectProps = {
  label: string;
  description?: string;
  value: ThemeMode | SurfaceTheme;
  options: readonly { value: ThemeMode | SurfaceTheme; label: string }[];
  onChange: (value: ThemeMode | SurfaceTheme) => void;
};

const surfaceThemeOptions = [
  { value: "follow", label: "跟随全局" },
  { value: "dark", label: "深色" },
  { value: "light", label: "浅色" },
] as const satisfies readonly { value: SurfaceTheme; label: string }[];

function ThemeSelect({ label, description, value, options, onChange }: ThemeSelectProps) {
  return (
    <div className="section">
      <label className="section-header">
        <span className="section-title">
          {label}
          {description && <small>{description}</small>}
        </span>
        <select
          aria-label={label}
          value={value}
          onChange={(event) => onChange(event.target.value as ThemeMode | SurfaceTheme)}
        >
          {options.map((option) => (
            <option key={option.value} value={option.value}>
              {option.label}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}

/** Shared appearance theme selectors for desktop and touch settings hosts. */
export function ThemeSettingsSection({
  preferences,
  mobile,
  linux,
  floatingToolbar,
  desktopPanels,
  onChange,
}: ThemeSettingsSectionProps) {
  return (
    <>
      <ThemeSelect
        label="主题模式"
        description="设置窗口和各界面的默认明暗模式"
        value={preferences.theme ?? "system"}
        options={[
          { value: "dark", label: "深色" },
          { value: "light", label: "浅色" },
          { value: "system", label: "跟随系统" },
        ]}
        onChange={(value) => onChange("theme", value)}
      />
      <ThemeSelect
        label="设置界面主题"
        description="覆盖主题模式，仅影响当前设置窗口"
        value={preferences.settings_theme ?? "follow"}
        options={surfaceThemeOptions}
        onChange={(value) => onChange("settings_theme", value)}
      />
      <ThemeSelect
        label={mobile ? "候选栏主题" : "候选窗口主题"}
        description={
          mobile
            ? "覆盖候选栏的明暗外观；跟随时使用键盘主题"
            : linux
              ? "预览跟随主题模式；IBus 候选窗与 Fcitx5 经典界面按此明暗着色"
              : "预览跟随主题模式"
        }
        value={preferences.candidate_theme ?? "follow"}
        options={surfaceThemeOptions}
        onChange={(value) => onChange("candidate_theme", value)}
      />
      {floatingToolbar && !linux && (
        <ThemeSelect
          label="悬浮工具栏主题"
          description="覆盖主题模式；当前影响工具栏设置预览，原生工具栏需宿主支持"
          value={preferences.toolbar_theme ?? "follow"}
          options={surfaceThemeOptions}
          onChange={(value) => onChange("toolbar_theme", value)}
        />
      )}
      {!mobile && !linux && (
        <ThemeSelect
          label="菜单主题"
          description="覆盖托盘菜单与候选右键菜单的明暗外观"
          value={preferences.menu_theme ?? "follow"}
          options={surfaceThemeOptions}
          onChange={(value) => onChange("menu_theme", value)}
        />
      )}
      <ThemeSelect
        label="表情面板主题"
        description="覆盖 Emoji、颜文字和符号面板的明暗外观"
        value={preferences.emoji_theme ?? "follow"}
        options={surfaceThemeOptions}
        onChange={(value) => onChange("emoji_theme", value)}
      />
      <ThemeSelect
        label="手写识别板主题"
        description="覆盖手写识别板的明暗外观"
        value={preferences.handwriting_theme ?? "follow"}
        options={surfaceThemeOptions}
        onChange={(value) => onChange("handwriting_theme", value)}
      />
      {desktopPanels && (
        <ThemeSelect
          label="语音输入弹出条主题"
          description="覆盖语音输入面板的明暗外观"
          value={preferences.voice_theme ?? "follow"}
          options={surfaceThemeOptions}
          onChange={(value) => onChange("voice_theme", value)}
        />
      )}
    </>
  );
}
