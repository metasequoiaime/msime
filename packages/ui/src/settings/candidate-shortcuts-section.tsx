import type { ReactNode } from "react";
import { SettingsGroupNote } from "./settings-group-note";
import * as settings from "./settings-style";
import { GroupList, MoreOptions, Row } from "../core/platform-controls";
import type { NavigationPreferences, WordCharacterPreferences } from "./word-character-section";
import { ShortcutRow } from "./shortcut-row";
import { SettingsShortcutKey } from "./settings-shortcut-key";
import { SwitchRow } from "./switch-row";

export interface CandidateShortcutsSectionProps {
  navigation: NavigationPreferences;
  /** 以词定字开着时列出它占用的键；不传就不列，例如没有实体键的 iOS。 */
  wordCharacter?: WordCharacterPreferences;
  numberRowSelection: boolean;
  showNumberRowSelection: boolean;
  mobile: boolean;
  onNumberRowSelectionChange: (value: boolean) => void;
  /** HarmonyOS 手机的「候选」分组：数字行开关和一行「恢复默认快捷键」，参考行收在「按键速查」分组下。 */
  harmonyPhone?: boolean;
  /** 把本页的所有快捷键恢复为默认值；由「恢复默认」按钮调用，该按钮只在 HarmonyOS 手机上显示。 */
  onRestoreDefaults?: () => void;
  /** HarmonyOS 手机的「翻页」分组内容（外接键盘翻页键），排在「候选」和「按键速查」之间。 */
  paging?: ReactNode;
}

/** 「向前 / 向后翻页」的各组按键，顺序与输入页的翻页方式一致。 */
const pagingKeys: [keyof NavigationPreferences, string][] = [
  ["minus_equal", "- / ="],
  ["comma_period", ", / ."],
  ["brackets", "[ / ]"],
  ["tab", "Shift+Tab / Tab"],
  ["page_up_down", "Page Up / Page Down"],
];

const wordCharacterKeys: Record<WordCharacterPreferences["keys"], string> = {
  brackets: "[ / ]",
  minus_equal: "- / =",
};

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
        <GroupList title="候选">
          {numberRowSwitch}
          {onRestoreDefaults && (
            <Row title="恢复默认快捷键">
              <button type="button" className={rowButton} onClick={onRestoreDefaults}>
                恢复默认
              </button>
            </Row>
          )}
        </GroupList>
        {paging && <GroupList title="翻页">{paging}</GroupList>}
        <GroupList title="按键速查">
          <MoreOptions>{reference}</MoreOptions>
        </GroupList>
      </>
    );
  }
  return (
    <GroupList title="候选操作">
      <SettingsGroupNote>输入和选取候选词时使用</SettingsGroupNote>
      {numberRowSwitch}
      {reference}
    </GroupList>
  );
}

/** 固定的参考行：显示候选时哪些键用于选择、翻页、移动和编辑。 */
function CandidateShortcutReference({
  navigation,
  wordCharacter,
  numberRowSelection,
  mobile,
}: Pick<
  CandidateShortcutsSectionProps,
  "navigation" | "wordCharacter" | "numberRowSelection" | "mobile"
>) {
  const paging = pagingKeys.filter(([option]) => navigation[option]);
  return (
    <>
      <ShortcutRow title="选择候选" chord={`Space${numberRowSelection ? " 或 1–9" : ""}`} />
      {/* 几组翻页键合成一行，免得同名的行重复出现。只有一组时和其他行一样放在行尾；多组时排到标题下方另起一行，标题不会被挤成两行。 */}
      {paging.length === 1 && <ShortcutRow title="向前 / 向后翻页" chord={paging[0][1]} />}
      {paging.length > 1 && (
        <Row
          title="向前 / 向后翻页"
          description={
            <span className={settings.shortcutKeys}>
              {paging.map(([option, chord]) => (
                <SettingsShortcutKey key={option}>{chord}</SettingsShortcutKey>
              ))}
            </span>
          }
        />
      )}
      {navigation.mouse_wheel && (
        <ShortcutRow title={mobile ? "候选栏翻页" : "候选窗口翻页"} chord="鼠标滚轮" />
      )}
      {navigation.arrows && <ShortcutRow title="移动候选项" chord="↑ / ↓" />}
      {wordCharacter?.enabled && (
        <ShortcutRow
          title="以词定字（上屏首字 / 末字）"
          chord={wordCharacterKeys[wordCharacter.keys]}
        />
      )}
      <ShortcutRow title="移动到候选列表首项 / 末项（页码随之切换）" chord="Home / End" />
      <ShortcutRow title="编辑输入串" chord="← / → / Backspace" />
      <ShortcutRow title="提交原始输入 / 取消输入" chord="Enter / Esc" />
    </>
  );
}
