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
  ["page_up_down", "PageUp / PageDown"],
];

/** HarmonyOS 手机把 legend 画成单独一行、只有标题，下面是两列复选框网格，所以这个块去掉行内边距，改给 legend。 */
const harmonyPhoneBlock =
  "min-w-0 [&_legend]:mb-0 [&_legend]:box-border [&_legend]:flex [&_legend]:min-h-[var(--p-row-h)] [&_legend]:w-full [&_legend]:items-center [&_legend]:[padding:var(--p-row-pad)]";

const linuxWheelPagingNote =
  "鼠标滚轮：开启后在 IBus 候选窗口上滚动即翻页，关闭时滚轮不做任何事。Fcitx5 经典界面的滚轮翻页是 Fcitx5 自己的设置，开启或改回关闭后会同步写入，对 Fcitx5 中的所有输入法生效；从未开启过时沿用 Fcitx5 原有设置。";

export interface NavigationSectionProps {
  navigation: NavigationPreferences;
  wordCharacter: WordCharacterPreferences;
  linux: boolean;
  /** HarmonyOS 手机的「候选栏」页以「外接键盘翻页键」承载翻页键：用设计的标签，没有鼠标滚轮，在只有标题的一行下面排成两列网格。 */
  harmonyPhone?: boolean;
  onChange: (next: {
    navigation: NavigationPreferences;
    wordCharacter: WordCharacterPreferences;
  }) => void;
}

/** 共用的候选翻页设置及其与以词定字的互斥：输入页「选词与翻页」组里以词定字之后的部分；HarmonyOS 手机上放在候选栏页的「翻页」组。 */
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
