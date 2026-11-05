import {
  capsLockLanguageSwitchDescription,
  harmonyCharacterSetOptions,
  harmonyLanguageSwitchOptions,
  languageSwitchChoice,
  languageSwitchOptions,
  languageSwitchPatch,
  type LanguageSwitchChoice,
} from "./input-mode-shortcuts-model";
import type { ReactNode } from "react";
import type { KeybindingPreferences } from "./keybinding-preferences";
import { defaultKeybindings } from "./keybinding-defaults";
import { SettingsGroupNote } from "./settings-group-note";
import { SettingsRowStack } from "./settings-row-stack";
import { InputModeHudSection } from "./input-mode-hud-section";
import { GroupList, Row } from "../core/platform-controls";
import { SwitchRow } from "./switch-row";
import { SelectRow } from "./select-row";

export interface InputModeShortcutPreferences extends KeybindingPreferences {
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
  /** 宿主实现了单按 Caps Lock 切换中英文（`HostCapabilities.caps_lock_language_switch`），显示它的开关。 */
  capsLockLanguageSwitch?: boolean;
  /** 宿主处理 Ctrl+Shift+J 并且本版本同时提供中文方案和日语（`offersJapaneseToggle`），显示它的开关。 */
  japaneseToggle?: boolean;
  /** 宿主把「切换中英文」接进中日英循环并且本版本同时提供中文方案和日语（`offersTrilingualCycle`），显示它的开关。 */
  trilingualCycle?: boolean;
  /** 整页快捷键的总说明（`ShortcutsIntroSection`），作为组内说明放在组首，和组说明一起收进组名旁的「?」。 */
  intro?: ReactNode;
  /** HarmonyOS 手机的「通用」分组：只列出其硬件按键路由处理的组合键，每项都是一个可选「无」的选择，并且没有分组说明。 */
  harmonyPhone?: boolean;
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
  capsLockLanguageSwitch = false,
  japaneseToggle = false,
  trilingualCycle = false,
  harmonyPhone = false,
  intro,
}: InputModeShortcutsSectionProps) {
  if (!showModeSwitchShortcuts) return null;
  if (harmonyPhone)
    return <HarmonyPhoneInputModeShortcuts keybindings={keybindings} onChange={onChange} />;

  const options = languageSwitchOptions(macos, linux);
  const characterSetLabel = macos ? "Control+Shift+F 切换繁体输出" : "Ctrl+Shift+F 切换繁体输出";

  return (
    <GroupList title="输入模式切换">
      <SettingsRowStack role="group" aria-label="输入模式切换快捷键">
        {intro}
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
          {options.map(([choice, label]) => (
            <option key={choice} value={choice}>
              {label}
            </option>
          ))}
        </SelectRow>
        {macos && (
          <SwitchRow
            title="仅在可编辑焦点时单按 Shift 切换"
            description="默认关闭。开启后需在系统设置的「隐私与安全性 › 辅助功能」中授权水杉输入法；未授权或无法确认当前焦点可编辑时，单按 Shift 不切换中英文。部分应用可能不支持焦点确认。"
            checked={keybindings.switch_language_shift_requires_editable ?? false}
            onChange={(checked) => onChange({ switch_language_shift_requires_editable: checked })}
          />
        )}
        {linux && (
          <SettingsGroupNote>
            Ctrl+Space 未选用或关闭后水杉不处理此组合键；若 IBus 或 Fcitx5
            配置了同名全局快捷键，需在框架设置中另行关闭。
          </SettingsGroupNote>
        )}
        {/* 和上面的切换键互不排斥：Caps Lock 是另加的一个键，打开它不关掉 Shift 等已选的切换键。 */}
        {capsLockLanguageSwitch && (
          <SwitchRow
            title="单按 Caps Lock 切换中英文"
            description={capsLockLanguageSwitchDescription(macos, linux)}
            checked={keybindings.switch_language_caps_lock ?? false}
            onChange={(checked) => onChange({ switch_language_caps_lock: checked })}
          />
        )}
        {/* 只有 Windows 和 Linux 处理它，所以按键名不分 macOS 的写法。 */}
        {japaneseToggle && (
          <SwitchRow
            title="Ctrl+Shift+J 在中文和日语之间切换"
            description="中文方案下按它切到日语，在日语里再按回到上次的中文方案；英文模式下交给应用。关着时这个组合键交给应用；浏览器里它是开发者工具或下载页的快捷键，打开后在浏览器里会被输入法占用。"
            checked={keybindings.toggle_japanese_ctrl_shift_j ?? false}
            onChange={(checked) => onChange({ toggle_japanese_ctrl_shift_j: checked })}
          />
        )}
        {/* 和上面的「切换中英文」是同一组键：打开后它们在中 → 日 → 英 → 中 之间循环，不是另加一个键。 */}
        {trilingualCycle && (
          <SwitchRow
            title="中英日循环切换"
            description="打开后上面的「切换中英文」快捷键按 中文 → 日文 → 英文 → 中文 依次切换，正在组字时先把已经打出来的原始字母上屏。英文模式下再按切换键回到中文；关着时这一组快捷键仍只在中文和英文之间切换。"
            checked={keybindings.trilingual_cycle ?? false}
            onChange={(checked) => onChange({ trilingual_cycle: checked })}
          />
        )}
        {/* 缺键时按开显示，与 client-core 的默认值一致。 */}
        <SwitchRow
          title={characterSetLabel}
          checked={
            keybindings.toggle_character_set_ctrl_shift_f ??
            defaultKeybindings.toggle_character_set_ctrl_shift_f
          }
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
      </SettingsRowStack>
    </GroupList>
  );
}

/** HarmonyOS 手机的「通用」组：中/英文切换和简繁切换各是一行选择，没有组说明。全角、标点、打开设置等组合键 Harmony 的按键路由不处理，所以不列。 */
function HarmonyPhoneInputModeShortcuts({
  keybindings,
  onChange,
}: Pick<InputModeShortcutsSectionProps, "keybindings" | "onChange">) {
  return (
    <GroupList title="通用">
      {/* 与其他宿主一样，只显示第一个已启用的组合键，用户选择之前不写入任何内容；「无」会关闭手机路由的所有组合键。 */}
      <SelectRow
        title="中/英文切换"
        value={languageSwitchChoice(keybindings, false)}
        onChange={(event) =>
          onChange(languageSwitchPatch(event.target.value as LanguageSwitchChoice, false))
        }
      >
        {harmonyLanguageSwitchOptions.map(([choice, label]) => (
          <option key={choice} value={choice}>
            {label}
          </option>
        ))}
      </SelectRow>
      <SelectRow
        title="简繁切换"
        value={keybindings.toggle_character_set_ctrl_shift_f ? "ctrl_shift_f" : "none"}
        onChange={(event) =>
          onChange({
            toggle_character_set_ctrl_shift_f: event.target.value === "ctrl_shift_f",
          })
        }
      >
        {harmonyCharacterSetOptions.map(([choice, label]) => (
          <option key={choice} value={choice}>
            {label}
          </option>
        ))}
      </SelectRow>
    </GroupList>
  );
}
