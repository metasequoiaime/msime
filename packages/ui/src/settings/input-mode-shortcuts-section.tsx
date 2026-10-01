import * as settings from "./settings-style";
import { InputModeHudSection } from "./input-mode-hud-section";
import { GroupList, Row, Switch } from "../core/platform-controls";

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

  // Named for the keys the user is actually looking at: macOS calls them Control and Option.
  const modeSwitchShortcutRows: [keyof InputModeShortcutPreferences, string][] = [
    ["switch_language_shift", "Shift 切换中英文"],
    ["switch_language_ctrl", macos ? "单击 Control 切换中英文" : "单击 Ctrl 切换中英文"],
    [
      "switch_language_ctrl_alt_space",
      macos ? "Control+Option+Space 切换中英文" : "Ctrl+Alt+Space 切换中英文",
    ],
    [
      "toggle_character_set_ctrl_shift_f",
      macos ? "Control+Shift+F 切换繁体输出" : "Ctrl+Shift+F 切换繁体输出",
    ],
  ];

  return (
    <GroupList title="输入模式切换">
      <div className={settings.rowStack} role="group" aria-label="输入模式切换快捷键">
        <p className={settings.groupNote}>
          在当前输入上下文中切换中英文模式；关闭后快捷键会交给应用处理。
        </p>
        {modeSwitchShortcutRows.map(([binding, label]) => (
          <Row key={binding} title={label}>
            <Switch
              checked={keybindings[binding] ?? false}
              onChange={(checked) => onChange({ [binding]: checked })}
            />
          </Row>
        ))}
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
        {windows && (
          <div className={`${settings.groupBlock} ${settings.shortcutIntro}`}>
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
    </GroupList>
  );
}
