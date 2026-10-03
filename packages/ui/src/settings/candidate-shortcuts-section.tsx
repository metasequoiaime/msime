import { SettingsGroupNote } from "./settings-group-note";
import * as settings from "./settings-style";
import { GroupList, Row } from "../core/platform-controls";
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

/** Candidate selection and navigation shortcut summary shared by host settings pages. */
export function CandidateShortcutsSection({
  navigation,
  wordCharacter,
  numberRowSelection,
  showNumberRowSelection,
  mobile,
  onNumberRowSelectionChange,
}: CandidateShortcutsSectionProps) {
  const paging = pagingKeys.filter(([option]) => navigation[option]);
  return (
    <GroupList title="候选操作">
      <SettingsGroupNote>输入和选取候选词时使用</SettingsGroupNote>
      {showNumberRowSelection && (
        <SwitchRow
          title="数字键选词"
          description="关闭后，候选窗口显示时数字键仍交给当前应用。"
          checked={numberRowSelection}
          onChange={onNumberRowSelectionChange}
        />
      )}
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
    </GroupList>
  );
}
