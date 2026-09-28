import * as settings from "./settings-style";
import { SettingToggle } from "./setting-toggle";
import type { NavigationPreferences } from "./word-character-section";

export interface CandidateShortcutsSectionProps {
  navigation: NavigationPreferences;
  numberRowSelection: boolean;
  showNumberRowSelection: boolean;
  mobile: boolean;
  onNumberRowSelectionChange: (value: boolean) => void;
}

/** Candidate selection and navigation shortcut summary shared by host settings pages. */
export function CandidateShortcutsSection({
  navigation,
  numberRowSelection,
  showNumberRowSelection,
  mobile,
  onNumberRowSelectionChange,
}: CandidateShortcutsSectionProps) {
  return (
    <div className={`section ${settings.shortcutSectionTitle}`}>
      <div className="section-title">候选操作</div>
      <small>输入和选取候选词时使用</small>
      {showNumberRowSelection && (
        <SettingToggle
          label="数字键选词"
          description="关闭后，候选窗口显示时数字键仍交给当前应用。"
          ariaLabel="数字键选词"
          checked={numberRowSelection}
          compact
          onChange={onNumberRowSelectionChange}
        />
      )}
      <div className={settings.shortcutList}>
        <div className={settings.shortcutRow}>
          <span>选择候选</span>
          <kbd>Space{numberRowSelection ? " 或 1–9" : ""}</kbd>
        </div>
        {navigation.minus_equal && (
          <div className={settings.shortcutRow}>
            <span>向前 / 向后翻页</span>
            <kbd>- / =</kbd>
          </div>
        )}
        {navigation.comma_period && (
          <div className={settings.shortcutRow}>
            <span>向前 / 向后翻页</span>
            <kbd>, / .</kbd>
          </div>
        )}
        {navigation.tab && (
          <div className={settings.shortcutRow}>
            <span>向前 / 向后翻页</span>
            <kbd>Shift+Tab / Tab</kbd>
          </div>
        )}
        {navigation.page_up_down && (
          <div className={settings.shortcutRow}>
            <span>向前 / 向后翻页</span>
            <kbd>Page Up / Page Down</kbd>
          </div>
        )}
        {navigation.mouse_wheel && (
          <div className={settings.shortcutRow}>
            <span>{mobile ? "候选栏翻页" : "候选窗口翻页"}</span>
            <kbd>鼠标滚轮</kbd>
          </div>
        )}
        {navigation.arrows && (
          <div className={settings.shortcutRow}>
            <span>移动候选项</span>
            <kbd>↑ / ↓</kbd>
          </div>
        )}
        <div className={settings.shortcutRow}>
          <span>移动到候选列表首项 / 末项（页码随之切换）</span>
          <kbd>Home / End</kbd>
        </div>
        <div className={settings.shortcutRow}>
          <span>编辑输入串</span>
          <kbd>← / → / Backspace</kbd>
        </div>
        <div className={settings.shortcutRow}>
          <span>提交原始输入 / 取消输入</span>
          <kbd>Enter / Esc</kbd>
        </div>
      </div>
    </div>
  );
}
