import type { ReactNode } from "react";
import { SettingsGroupNote } from "./settings-group-note";
import * as settings from "./settings-style";
import { GroupList, MoreOptions, Row } from "../core/platform-controls";
import type {
  NavigationPreferences,
  WordCharacterPreferences,
} from "./candidate-paging-preferences";
import { navigationWithDefaults } from "./candidate-paging-preferences";
import { ShortcutRow } from "./shortcut-row";
import { SettingsShortcutKey } from "./settings-shortcut-key";
import { SwitchRow } from "./switch-row";
import { enabledPagingShortcuts, wordCharacterKeys } from "./candidate-paging-options";

export interface CandidateShortcutsSectionProps {
  navigation: NavigationPreferences;
  /** 以词定字开着时列出它占用的键；不传就不列，例如没有实体键的 iOS。 */
  wordCharacter?: WordCharacterPreferences;
  numberRowSelection: boolean;
  showNumberRowSelection: boolean;
  mobile: boolean;
  onNumberRowSelectionChange: (value: boolean) => void;
  /** HarmonyOS 手机的「候选」分组只有数字行开关，参考行收在「按键速查」分组下，「恢复默认快捷键」单独成组排在页末。 */
  harmonyPhone?: boolean;
  /** 把本页的所有快捷键（含翻页键）恢复为默认值；由「恢复默认」按钮调用，该按钮只在 HarmonyOS 手机上显示。 */
  onRestoreDefaults?: () => void;
  /** HarmonyOS 手机的「翻页」分组内容（外接键盘翻页键），排在「候选」和「按键速查」之间。 */
  paging?: ReactNode;
  /** 接在候选参考行之后、一起收进「按键速查」的其他只读行（面板快捷键）。 */
  extraReference?: ReactNode;
  /** 桌面将速查放进独立标签时，这里只保留可编辑的候选操作。 */
  showReference?: boolean;
}

/** 设计稿中的行内按钮：胶囊形，使用平台的按钮 token，13px 文字。 */
const rowButton =
  "shrink-0 cursor-pointer rounded-[var(--p-r-ctl)] px-3.5 py-[5px] text-[13px] whitespace-nowrap [background:var(--p-btn-bg)] [border:var(--p-btn-border)] [color:var(--p-btn-fg)] [font-family:inherit] active:opacity-60 disabled:cursor-default disabled:opacity-50";

/** Candidate selection and navigation shortcut summary shared by host settings pages. */
export function CandidateShortcutsSection({
  navigation,
  wordCharacter,
  numberRowSelection,
  showNumberRowSelection,
  mobile,
  onNumberRowSelectionChange,
  harmonyPhone = false,
  onRestoreDefaults,
  paging,
  extraReference,
  showReference = true,
}: CandidateShortcutsSectionProps) {
  const numberRowSwitch = showNumberRowSelection && (
    <SwitchRow
      title="数字键选词"
      description={
        harmonyPhone
          ? "关闭后，候选栏显示时数字键仍交给当前应用。"
          : "关闭后，候选窗口显示时数字键仍交给当前应用。"
      }
      checked={numberRowSelection}
      onChange={onNumberRowSelectionChange}
    />
  );
  const reference = (
    <CandidateShortcutReference
      navigation={navigation}
      wordCharacter={wordCharacter}
      numberRowSelection={numberRowSelection}
      mobile={mobile}
    />
  );
  if (harmonyPhone) {
    return (
      <>
        <GroupList title="候选">{numberRowSwitch}</GroupList>
        {paging && <GroupList title="翻页">{paging}</GroupList>}
        <GroupList title="按键速查">
          <MoreOptions>{reference}</MoreOptions>
        </GroupList>
        {/* 恢复的是整页（通用、候选和翻页），所以不放进其中任何一组，单独排在页末。 */}
        {onRestoreDefaults && (
          <GroupList>
            <Row title="恢复默认快捷键">
              <button type="button" className={rowButton} onClick={onRestoreDefaults}>
                恢复默认
              </button>
            </Row>
          </GroupList>
        )}
      </>
    );
  }
  // 参考行不是设置，却占了这一页的大半；和鸿蒙手机一样收进「按键速查」。翻页键和以词定字在输入页设置，这里只列出它们现在用哪些键。
  return (
    <>
      {numberRowSwitch && (
        <GroupList title="候选操作">
          <SettingsGroupNote>输入和选取候选词时使用</SettingsGroupNote>
          {numberRowSwitch}
        </GroupList>
      )}
      {showReference && (
        <GroupList title="按键速查">
          <MoreOptions>
            {reference}
            {extraReference}
          </MoreOptions>
        </GroupList>
      )}
    </>
  );
}

/** 固定的参考行：显示候选时哪些键用于选择、翻页、移动和编辑。 */
export function CandidateShortcutReference({
  navigation,
  wordCharacter,
  numberRowSelection,
  mobile,
}: Pick<
  CandidateShortcutsSectionProps,
  "navigation" | "wordCharacter" | "numberRowSelection" | "mobile"
>) {
  const effectiveNavigation = navigationWithDefaults(navigation);
  const paging = enabledPagingShortcuts(effectiveNavigation);
  return (
    <>
      <ShortcutRow title="选择候选" chord={`Space${numberRowSelection ? " 或 1–9" : ""}`} />
      {/* 几组翻页键合成一行，免得同名的行重复出现。只有一组时和其他行一样放在行尾；多组时排到标题下方另起一行，标题不会被挤成两行。 */}
      {paging.length === 1 && <ShortcutRow title="向前 / 向后翻页" chord={paging[0][1]} />}
      {paging.length > 1 && (
        <Row
          title="向前 / 向后翻页"
          detail={
            <span className={settings.shortcutKeys}>
              {paging.map(([option, chord]) => (
                <SettingsShortcutKey key={option}>{chord}</SettingsShortcutKey>
              ))}
            </span>
          }
        />
      )}
      {effectiveNavigation.mouse_wheel && (
        <ShortcutRow title={mobile ? "候选栏翻页" : "候选窗口翻页"} chord="鼠标滚轮" />
      )}
      {effectiveNavigation.arrows && <ShortcutRow title="移动候选项" chord="↑ / ↓" />}
      {/* 打开左右键移动候选时，只有横排候选窗里左右键移高亮，而且首选上仍编辑输入串（→ 到末尾为止），所以两行标题都要写清是什么时候发生的。竖排时左右键只编辑输入串。 */}
      {effectiveNavigation.left_right && (
        <ShortcutRow title="移动候选项（横排候选，首选上先编辑输入串）" chord="← / →" />
      )}
      {wordCharacter?.enabled && (
        <ShortcutRow
          title="以词定字（上屏首字 / 末字）"
          chord={wordCharacterKeys[wordCharacter.keys]}
        />
      )}
      {/* 桌面宿主的 Home 分两段：高亮已在首项时再按一次，把输入串的光标移到开头。触屏宿主的外接键盘没有第二段。 */}
      <ShortcutRow
        title={
          mobile
            ? "移动到候选列表首项 / 末项（页码随之切换）"
            : "移动到候选列表首项 / 末项（页码随之切换；已在首项时 Home 把光标移到输入串开头）"
        }
        chord="Home / End"
      />
      <ShortcutRow title="编辑输入串" chord="← / → / Backspace" />
      {/* 桌面三个宿主组字时 Shift+左右按音节移动输入串的光标，高亮停在哪个候选上都一样；触屏宿主的外接键盘没有这个组合键。Windows 和 Linux 的 Ctrl+左右相同，macOS 的 Ctrl+左右默认被系统用来切换桌面，所以这里只列 Shift。 */}
      {!mobile && <ShortcutRow title="按音节移动输入串光标" chord="Shift + ← / →" />}
      <ShortcutRow title="提交原始输入 / 取消输入" chord="Enter / Esc" />
    </>
  );
}
