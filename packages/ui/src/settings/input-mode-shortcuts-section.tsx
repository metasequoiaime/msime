import { SettingsGroupNote } from "./settings-group-note";
import * as settings from "./settings-style";
import { InputModeHudSection } from "./input-mode-hud-section";
import { GroupList, Row } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";
import { SelectRow } from "./select-row";

export interface InputModeShortcutPreferences {
  switch_language_shift: boolean;
  switch_language_ctrl: boolean;
  switch_language_ctrl_space?: boolean;
  switch_language_ctrl_alt_space: boolean;
  toggle_character_set_ctrl_shift_f: boolean;
  toggle_fullwidth_option_shift_h: boolean;
  input_mode_hud?: boolean;
}

export interface InputModeShortcutsSectionProps {
  keybindings: InputModeShortcutPreferences;
  onChange: (patch: Partial<InputModeShortcutPreferences>) => void;
  onInputModeHUDChange?: (value: boolean) => void;
  showModeSwitchShortcuts: boolean;
  macos: boolean;
  linux?: boolean;
  /** 设置窗口把中英文切换提示放在输入页，所以快捷键页不传；留给自己拼快捷键组、想把提示放在这里的宿主，只在 macOS 生效。 */
  showInputModeHUD?: boolean;
  inputModeHUD?: boolean;
  showFullwidthChord: boolean;
  fullwidthChord?: string;
  windows: boolean;
}

/** 中英文切换快捷键各占一项；Ctrl+Space 只在 Linux 提供，另有「不使用」。 */
type LanguageSwitchChoice = "shift" | "ctrl" | "ctrl_space" | "ctrl_alt_space" | "none";

type LanguageSwitchBinding =
  | "switch_language_shift"
  | "switch_language_ctrl"
  | "switch_language_ctrl_space"
  | "switch_language_ctrl_alt_space";

/** 同时开着多个时按选项顺序显示第一项，不在打开页面时改写偏好。 */
const languageSwitchBindings: [Exclude<LanguageSwitchChoice, "none">, LanguageSwitchBinding][] = [
  ["shift", "switch_language_shift"],
  ["ctrl", "switch_language_ctrl"],
  ["ctrl_space", "switch_language_ctrl_space"],
  ["ctrl_alt_space", "switch_language_ctrl_alt_space"],
];

/** Linux 旧配置缺少 Ctrl+Space 字段时仍默认开启；其余平台不读取这一项。 */
function languageSwitchChoice(
  keybindings: InputModeShortcutPreferences,
  linux: boolean,
): LanguageSwitchChoice {
  return (
    languageSwitchBindings.find(([, binding]) =>
      binding === "switch_language_ctrl_space"
        ? linux && (keybindings[binding] ?? true)
        : keybindings[binding],
    )?.[0] ?? "none"
  );
}

/** 用户选择时一次更新当前平台支持的所有切换键；「不使用」全部关闭。 */
function languageSwitchPatch(
  choice: LanguageSwitchChoice,
  linux: boolean,
): Pick<InputModeShortcutPreferences, LanguageSwitchBinding> {
  return {
    switch_language_shift: choice === "shift",
    switch_language_ctrl: choice === "ctrl",
    ...(linux ? { switch_language_ctrl_space: choice === "ctrl_space" } : {}),
    switch_language_ctrl_alt_space: choice === "ctrl_alt_space",
  };
}

/** Shortcut toggles shared by hosts that expose input mode controls. */
export function InputModeShortcutsSection({
  keybindings,
  onChange,
  onInputModeHUDChange,
  showModeSwitchShortcuts,
  macos,
  linux = false,
  showInputModeHUD = false,
  inputModeHUD = false,
  showFullwidthChord,
  fullwidthChord = "Alt+Shift+H",
  windows,
}: InputModeShortcutsSectionProps) {
  if (!showModeSwitchShortcuts) return null;

  // 按宿主显示按键名称：macOS 使用 Control 和 Option。
  const languageSwitchOptions: [LanguageSwitchChoice, string][] = [
    ["shift", "Shift"],
    ["ctrl", macos ? "单击 Control" : "单击 Ctrl"],
    ["ctrl_alt_space", macos ? "Control+Option+Space" : "Ctrl+Alt+Space"],
    ["none", "不使用"],
  ];
  if (linux) languageSwitchOptions.splice(2, 0, ["ctrl_space", "Ctrl+Space"]);
  const characterSetLabel = macos ? "Control+Shift+F 切换繁体输出" : "Ctrl+Shift+F 切换繁体输出";

  return (
    <GroupList title="输入模式切换">
      <div className={settings.rowStack} role="group" aria-label="输入模式切换快捷键">
        <SettingsGroupNote>
          在当前输入上下文中切换中英文模式；未选用或关闭的快捷键会交给应用处理。
        </SettingsGroupNote>
        {/* 各页始终挂载，挂载时写回会覆盖尚未读完的偏好及其他窗口的写入。这里只显示优先项，保留默认同时开启的 Shift、Control+Option+Space 和 Linux Ctrl+Space；用户选择时才一次更新全部支持的切换键。 */}
        <SelectRow
          title="切换中英文"
          value={languageSwitchChoice(keybindings, linux)}
          onChange={(event) =>
            onChange(languageSwitchPatch(event.target.value as LanguageSwitchChoice, linux))
          }
        >
          {languageSwitchOptions.map(([choice, label]) => (
            <option key={choice} value={choice}>
              {label}
            </option>
          ))}
        </SelectRow>
        {linux && (
          <SettingsGroupNote>
            Ctrl+Space 未选用或关闭后水杉不处理此组合键；若 IBus 或 Fcitx5
            配置了同名全局快捷键，需在框架设置中另行关闭。
          </SettingsGroupNote>
        )}
        <SwitchRow
          title={characterSetLabel}
          checked={keybindings.toggle_character_set_ctrl_shift_f ?? false}
          onChange={(checked) => onChange({ toggle_character_set_ctrl_shift_f: checked })}
        />
        {macos && showInputModeHUD && (
          <InputModeHudSection
            shortcut
            value={inputModeHUD}
            onChange={(value) => onInputModeHUDChange?.(value)}
          />
        )}
        {showFullwidthChord && (
          <SwitchRow
            title={`${fullwidthChord} 切换全半角`}
            description="关掉后这个组合键交给应用处理；工具栏的全半角开关不受影响。"
            checked={keybindings.toggle_fullwidth_option_shift_h}
            onChange={(checked) => onChange({ toggle_fullwidth_option_shift_h: checked })}
          />
        )}
        {/* Ctrl+Space 是 Windows 自己的快捷键，这里只说明去哪里改，压成组末一行。 */}
        {windows && (
          <Row
            title="Ctrl+Space（由 Windows 管理）"
            description="此处不控制。要修改或关闭，打开「设置 › 时间和语言 › 输入 › 高级键盘设置 › 输入语言热键」，选中「中文（简体）输入法 - 输入法 / 非输入法切换」，点「更改按键顺序」后关闭它或改成不常用的组合。不同 Windows 版本的名称可能略有差异；未立即生效时请重新登录或重启电脑。"
          />
        )}
      </div>
    </GroupList>
  );
}
