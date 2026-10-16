import { SettingsGroupNote } from "./settings-group-note";
import { Checks } from "../core/platform-controls";
import {
  pagingShortcutChange,
  type NavigationPreferences,
  type WordCharacterPreferences,
  type CandidatePagingChange,
} from "./candidate-paging-preferences";
import { SettingsGroupBlock } from "./settings-group-block";
import { linuxWheelPagingNote, navigationShortcutItems } from "./candidate-paging-options";
export { defaultNavigation } from "./candidate-paging-preferences";

/** HarmonyOS 手机上外面的「翻页」组标题已经说明了这是什么，legend 只留给读屏，不再画成一行和组标题重复；两列复选框网格自己补上顶部留白。 */
const harmonyPhoneBlock = "min-w-0 [&_legend]:sr-only [&>fieldset>div]:pt-3";

export interface NavigationSectionProps {
  navigation: NavigationPreferences;
  wordCharacter: WordCharacterPreferences;
  linux: boolean;
  /** HarmonyOS 手机的「外接键盘快捷键」页以「外接键盘翻页键」承载翻页键：用设计的标签，没有鼠标滚轮，直接在「翻页」组里排成两列网格，legend 只给读屏。 */
  harmonyPhone?: boolean;
  /** 宿主读 Tab 和 PageUp/PageDown 两组翻页键（`HostCapabilities.tab_page_paging_switches`）。为假时（macOS、Windows、HarmonyOS）两组固定开启，不列出它们的复选框。缺省为真。 */
  tabPageSwitches?: boolean;
  onChange: (next: CandidatePagingChange) => void;
}

/** 共用的候选翻页设置及其与以词定字的互斥：输入页「选词与翻页」组里以词定字之后的部分；HarmonyOS 手机上放在「外接键盘快捷键」页的「翻页」组。 */
export function NavigationSection({
  navigation,
  wordCharacter,
  linux,
  harmonyPhone = false,
  tabPageSwitches = true,
  onChange,
}: NavigationSectionProps) {
  const items = navigationShortcutItems(navigation, harmonyPhone).filter(
    ({ value }) => tabPageSwitches || (value !== "tab" && value !== "page_up_down"),
  );
  const checks = (
    <Checks
      legend={harmonyPhone ? "外接键盘翻页键" : "翻页方式"}
      layout={harmonyPhone ? "grid" : "list"}
      description={
        tabPageSwitches
          ? undefined
          : `Shift+Tab / Tab 和 PageUp / PageDown 在${harmonyPhone ? "候选栏" : "候选窗口"}显示时始终翻页`
      }
      items={items}
      onChange={(key, enabled) =>
        onChange(pagingShortcutChange(navigation, wordCharacter, key, enabled))
      }
    />
  );
  if (harmonyPhone) return <div className={harmonyPhoneBlock}>{checks}</div>;
  return (
    <>
      <SettingsGroupBlock>{checks}</SettingsGroupBlock>
      {/* IBus pages on the panel's cursor_up/down and button 4/5 only with the switch on; Fcitx5 classic UI pages by itself, so the host writes the switch into classicui's WheelForPaging once it leaves the default (platforms/linux/README.md). */}
      {linux && <SettingsGroupNote>{linuxWheelPagingNote}</SettingsGroupNote>}
    </>
  );
}
