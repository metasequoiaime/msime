import * as settings from "./settings-style";
import { GroupList, Row } from "../core/platform-controls";
import type { NavigationPreferences } from "./word-character-section";
import { SwitchRow } from "./switch-row";

export interface CandidateShortcutsSectionProps {
  navigation: NavigationPreferences;
  numberRowSelection: boolean;
  showNumberRowSelection: boolean;
  mobile: boolean;
  onNumberRowSelectionChange: (value: boolean) => void;
}

const key = (chord: string) => <kbd className={settings.shortcutKey}>{chord}</kbd>;

/** Candidate selection and navigation shortcut summary shared by host settings pages. */
export function CandidateShortcutsSection({
  navigation,
  numberRowSelection,
  showNumberRowSelection,
  mobile,
  onNumberRowSelectionChange,
}: CandidateShortcutsSectionProps) {
  return (
    <GroupList title="候选操作">
      <p className={settings.groupNote}>输入和选取候选词时使用</p>
      {showNumberRowSelection && (
        <SwitchRow
          title="数字键选词"
          description="关闭后，候选窗口显示时数字键仍交给当前应用。"
          checked={numberRowSelection}
          onChange={onNumberRowSelectionChange}
        />
      )}
      <Row title="选择候选">{key(`Space${numberRowSelection ? " 或 1–9" : ""}`)}</Row>
      {navigation.minus_equal && <Row title="向前 / 向后翻页">{key("- / =")}</Row>}
      {navigation.comma_period && <Row title="向前 / 向后翻页">{key(", / .")}</Row>}
      {navigation.tab && <Row title="向前 / 向后翻页">{key("Shift+Tab / Tab")}</Row>}
      {navigation.page_up_down && <Row title="向前 / 向后翻页">{key("Page Up / Page Down")}</Row>}
      {navigation.mouse_wheel && (
        <Row title={mobile ? "候选栏翻页" : "候选窗口翻页"}>{key("鼠标滚轮")}</Row>
      )}
      {navigation.arrows && <Row title="移动候选项">{key("↑ / ↓")}</Row>}
      <Row title="移动到候选列表首项 / 末项（页码随之切换）">{key("Home / End")}</Row>
      <Row title="编辑输入串">{key("← / → / Backspace")}</Row>
      <Row title="提交原始输入 / 取消输入">{key("Enter / Esc")}</Row>
    </GroupList>
  );
}
