import * as settings from "./settings-style";
import { InputModeHudSection } from "./input-mode-hud-section";

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
  showInputModeHUD: boolean;
  inputModeHUD: boolean;
  showFullwidthChord: boolean;
  fullwidthChord?: string;
  windows: boolean;
}

/** Shortcut toggles shared by hosts that expose input mode controls. */
export function InputModeShortcutsSection({
  keybindings,
  onChange,
  onInputModeHUDChange,
  showModeSwitchShortcuts,
  macos,
  showInputModeHUD,
  inputModeHUD,
  showFullwidthChord,
  fullwidthChord = "Alt+Shift+H",
  windows,
}: InputModeShortcutsSectionProps) {
  if (!showModeSwitchShortcuts) return null;

  const modeSwitchShortcutRows: [keyof InputModeShortcutPreferences, string][] = [
    ["switch_language_shift", "Shift 切换中英文"],
    ["switch_language_ctrl", macos ? "单击 Control 切换中英文" : "单击 Ctrl 切换中英文"],
    [
      "switch_language_ctrl_alt_space",
      macos ? "Control+Option+Space 切换中英文" : "Ctrl+Alt+Space 切换中英文",
    ],
    [
      "toggle_character_set_ctrl_shift_f",
      macos ? "Control+Shift+F 切换简繁" : "Ctrl+Shift+F 切换简繁",
    ],
  ];

  return (
    <div className="section" role="group" aria-label="输入模式切换快捷键">
      <div className="section-title">输入模式切换</div>
      <small>在当前输入上下文中切换中英文模式；关闭后快捷键会交给应用处理。</small>
      {modeSwitchShortcutRows.map(([key, label]) => (
        <label className="section-header" key={key}>
          <span className="section-title">{label}</span>
          <input
            aria-label={label}
            className="toggle"
            type="checkbox"
            checked={keybindings[key]}
            onChange={(event) => onChange({ [key]: event.target.checked })}
          />
        </label>
      ))}
      {macos && showInputModeHUD && (
        <InputModeHudSection
          shortcut
          value={inputModeHUD}
          onChange={(value) => onInputModeHUDChange?.(value)}
        />
      )}
      {showFullwidthChord && (
        <label className="section-header">
          <span className="section-title">
            {fullwidthChord} 切换全半角
            <small>关掉后这个组合键交给应用处理；工具栏的全半角开关不受影响。</small>
          </span>
          <input
            aria-label={`${fullwidthChord} 切换全半角`}
            className="toggle"
            type="checkbox"
            checked={keybindings.toggle_fullwidth_option_shift_h}
            onChange={(event) =>
              onChange({ toggle_fullwidth_option_shift_h: event.target.checked })
            }
          />
        </label>
      )}
      {windows && (
        <div className={settings.shortcutIntro}>
          <div className="section-title">修改或关闭 Ctrl+Space（系统）</div>
          <small>
            Ctrl+Space 由 Windows 管理，此处不控制。请前往系统设置修改“输入法 /
            非输入法切换”的按键顺序：
          </small>
          <ol>
            <li>打开“设置”，进入“时间和语言” → “输入”。</li>
            <li>选择“高级键盘设置” → “输入语言热键”。</li>
            <li>选中“中文（简体）输入法 - 输入法 / 非输入法切换”，点击“更改按键顺序”。</li>
            <li>关闭该按键顺序，或将 Ctrl+Space 改为其他不常用组合。</li>
          </ol>
          <small>
            不同 Windows 版本的选项名称可能略有差异；修改后如未立即生效，请重新登录或重启电脑。
          </small>
        </div>
      )}
    </div>
  );
}
