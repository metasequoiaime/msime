import { SettingsGroupNote } from "./settings-group-note";
import { Checks } from "../core/platform-controls";
import type { NavigationPreferences, WordCharacterPreferences } from "./word-character-section";
import { SettingsGroupBlock } from "./settings-group-block";

export const defaultNavigation: NavigationPreferences = {
  minus_equal: true,
  comma_period: true,
  brackets: false,
  tab: true,
  page_up_down: true,
  arrows: true,
};

const navigationOptions: [keyof NavigationPreferences, string][] = [
  ["minus_equal", "- / ="],
  ["comma_period", ", / ."],
  ["brackets", "[ / ]"],
  ["tab", "Shift+Tab / Tab"],
  ["page_up_down", "PageUp / PageDown"],
  ["mouse_wheel", "鼠标滚轮（候选窗口支持时翻页）"],
  ["arrows", "上 / 下（移动候选项）"],
];

/** HarmonyOS 手机的翻页键，按设计的顺序和标签。手机没有鼠标滚轮，所以不提供该选项；已保存的值保持不变。 */
const harmonyPhoneNavigationOptions: [keyof NavigationPreferences, string][] = [
  ["minus_equal", "- / ="],
  ["comma_period", "，/ 。"],
  ["brackets", "[ / ]"],
  ["arrows", "↑ / ↓"],
  ["tab", "Shift+Tab / Tab"],
  // 两列网格里放不下「PageUp / PageDown」，用键帽上常印的缩写，免得折成两行。
  ["page_up_down", "PgUp / PgDn"],
];

/** HarmonyOS 手机上外面的「翻页」组标题已经说明了这是什么，legend 只留给读屏，不再画成一行和组标题重复；两列复选框网格自己补上顶部留白。 */
const harmonyPhoneBlock = "min-w-0 [&_legend]:sr-only [&>fieldset>div]:pt-3";

const linuxWheelPagingNote =
  "鼠标滚轮：开启后在 IBus 候选窗口上滚动即翻页，关闭时滚轮不做任何事。Fcitx5 经典界面的滚轮翻页是 Fcitx5 自己的设置，开启或改回关闭后会同步写入，对 Fcitx5 中的所有输入法生效；从未开启过时沿用 Fcitx5 原有设置。";

export interface NavigationSectionProps {
  navigation: NavigationPreferences;
  wordCharacter: WordCharacterPreferences;
  linux: boolean;
  /** HarmonyOS 手机的「外接键盘快捷键」页以「外接键盘翻页键」承载翻页键：用设计的标签，没有鼠标滚轮，直接在「翻页」组里排成两列网格，legend 只给读屏。 */
  harmonyPhone?: boolean;
  onChange: (next: {
    navigation: NavigationPreferences;
    wordCharacter: WordCharacterPreferences;
  }) => void;
}

/** 共用的候选翻页设置及其与以词定字的互斥：输入页「选词与翻页」组里以词定字之后的部分；HarmonyOS 手机上放在「外接键盘快捷键」页的「翻页」组。 */
export function NavigationSection({
  navigation,
  wordCharacter,
  linux,
  harmonyPhone = false,
  onChange,
}: NavigationSectionProps) {
  const checks = (
    <Checks
      legend={harmonyPhone ? "外接键盘翻页键" : "翻页方式"}
      layout={harmonyPhone ? "grid" : "list"}
      items={(harmonyPhone ? harmonyPhoneNavigationOptions : navigationOptions).map(
        ([key, label]) => ({
          value: key,
          label,
          // `mouse_wheel` 是可选的；未设置即关闭，与一直以来的显示一致。
          checked: navigation[key] ?? false,
        }),
      )}
      onChange={(key, enabled) =>
        onChange({
          navigation: { ...navigation, [key]: enabled },
          wordCharacter:
            enabled && wordCharacter.enabled && wordCharacter.keys === key
              ? { ...wordCharacter, enabled: false }
              : wordCharacter,
        })
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
