import * as settings from "./settings-style";
import { InputModeHudSection } from "./input-mode-hud-section";
import { GroupList, Row, Select, Switch } from "../core/platform-controls";

export interface InputModeShortcutPreferences {
  switch_language_shift: boolean;
  switch_language_ctrl: boolean;
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
  /** 设置窗口把中英文切换提示放在输入页，所以快捷键页不传；留给自己拼快捷键组、想把提示放在这里的宿主，只在 macOS 生效。 */
  showInputModeHUD?: boolean;
  inputModeHUD?: boolean;
  showFullwidthChord: boolean;
  fullwidthChord?: string;
  windows: boolean;
}

/** 「切换中英文」下拉框的选项：三种快捷键各占一项，另有「不使用」。 */
type LanguageSwitchChoice = "shift" | "ctrl" | "ctrl_alt_space" | "none";

type LanguageSwitchBinding =
  | "switch_language_shift"
  | "switch_language_ctrl"
  | "switch_language_ctrl_alt_space";

/** 下拉框的顺序，也是同时开着多个时保留哪一个的优先级：Shift > Control > Control+Option+Space。 */
const languageSwitchBindings: [Exclude<LanguageSwitchChoice, "none">, LanguageSwitchBinding][] = [
  ["shift", "switch_language_shift"],
  ["ctrl", "switch_language_ctrl"],
  ["ctrl_alt_space", "switch_language_ctrl_alt_space"],
];

/** 偏好里仍是三个独立的布尔值；同时开着多个时按优先级显示第一个，全关时是「不使用」。 */
function languageSwitchChoice(keybindings: InputModeShortcutPreferences): LanguageSwitchChoice {
  return languageSwitchBindings.find(([, binding]) => keybindings[binding])?.[0] ?? "none";
}

/** 选中一项就把对应的布尔值设为真、另外两个设为假，一次写全三个；「不使用」三个都设为假。 */
function languageSwitchPatch(
  choice: LanguageSwitchChoice,
): Pick<InputModeShortcutPreferences, LanguageSwitchBinding> {
  return {
    switch_language_shift: choice === "shift",
    switch_language_ctrl: choice === "ctrl",
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
  showInputModeHUD = false,
  inputModeHUD = false,
  showFullwidthChord,
  fullwidthChord = "Alt+Shift+H",
  windows,
}: InputModeShortcutsSectionProps) {
  if (!showModeSwitchShortcuts) return null;

  // Named for the keys the user is actually looking at: macOS calls them Control and Option.
  const languageSwitchOptions: [LanguageSwitchChoice, string][] = [
    ["shift", "Shift"],
    ["ctrl", macos ? "单击 Control" : "单击 Ctrl"],
    ["ctrl_alt_space", macos ? "Control+Option+Space" : "Ctrl+Alt+Space"],
    ["none", "不使用"],
  ];
  const characterSetLabel = macos ? "Control+Shift+F 切换繁体输出" : "Ctrl+Shift+F 切换繁体输出";

  return (
    <GroupList title="输入模式切换">
      <div className={settings.rowStack} role="group" aria-label="输入模式切换快捷键">
        <p className={settings.groupNote}>
          在当前输入上下文中切换中英文模式；未选用或关闭的快捷键会交给应用处理。
        </p>
        {/* 同时开着多个的旧设置不在打开页面时改写：这个组件在页面隐藏时也挂着，偏好读完之前拿到的是默认值，而默认值本身就同时开着 Shift 和 Control+Option+Space，挂载时写回会让每个打开设置窗口的人都被静默改掉一项，还会和其他窗口、原生设置的写入互相覆盖。这里只按优先级显示一项，用户第一次在下拉框里选择时一次写全三个布尔值，多余的那几个随之关掉。 */}
        <Row title="切换中英文">
          <Select
            value={languageSwitchChoice(keybindings)}
            onChange={(event) =>
              onChange(languageSwitchPatch(event.target.value as LanguageSwitchChoice))
            }
          >
            {languageSwitchOptions.map(([choice, label]) => (
              <option key={choice} value={choice}>
                {label}
              </option>
            ))}
          </Select>
        </Row>
        <Row title={characterSetLabel}>
          <Switch
            checked={keybindings.toggle_character_set_ctrl_shift_f ?? false}
            onChange={(checked) => onChange({ toggle_character_set_ctrl_shift_f: checked })}
          />
        </Row>
        {macos && showInputModeHUD && (
          <InputModeHudSection
            shortcut
            value={inputModeHUD}
            onChange={(value) => onInputModeHUDChange?.(value)}
          />
        )}
        {showFullwidthChord && (
          <Row
            title={`${fullwidthChord} 切换全半角`}
            description="关掉后这个组合键交给应用处理；工具栏的全半角开关不受影响。"
          >
            <Switch
              checked={keybindings.toggle_fullwidth_option_shift_h}
              onChange={(checked) => onChange({ toggle_fullwidth_option_shift_h: checked })}
            />
          </Row>
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
