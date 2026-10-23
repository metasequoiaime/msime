import type {
  NavigationPreferences,
  WordCharacterPreferences,
} from "./candidate-paging-preferences";

type NavigationOption = readonly [keyof NavigationPreferences, string];

const navigationOptions: readonly NavigationOption[] = [
  ["minus_equal", "- / ="],
  ["comma_period", ", / ."],
  ["brackets", "[ / ]"],
  ["tab", "Shift+Tab / Tab"],
  ["page_up_down", "PageUp / PageDown"],
  ["mouse_wheel", "鼠标滚轮（候选窗口支持时翻页）"],
  ["arrows", "上 / 下（移动候选项）"],
  ["left_right", "左 / 右（移动候选项）"],
];

/** HarmonyOS 手机的翻页键，按设计的顺序和标签。手机没有鼠标滚轮，所以不提供该选项；已保存的值保持不变。 */
const harmonyPhoneNavigationOptions: readonly NavigationOption[] = [
  ["minus_equal", "- / ="],
  ["comma_period", "，/ 。"],
  ["brackets", "[ / ]"],
  ["arrows", "↑ / ↓"],
  ["left_right", "← / →"],
  ["tab", "Shift+Tab / Tab"],
  // 两列网格里放不下「PageUp / PageDown」，用键帽上常印的缩写，免得折成两行。
  ["page_up_down", "PgUp / PgDn"],
];

/** 「向前 / 向后翻页」的各组按键，顺序与输入页的翻页方式一致。 */
const pagingKeys: readonly NavigationOption[] = [
  ["minus_equal", "- / ="],
  ["comma_period", ", / ."],
  ["brackets", "[ / ]"],
  ["tab", "Shift+Tab / Tab"],
  ["page_up_down", "Page Up / Page Down"],
];

export const wordCharacterKeys: Readonly<Record<WordCharacterPreferences["keys"], string>> = {
  brackets: "[ / ]",
  minus_equal: "- / =",
};

const wordCharacterChoices = ["brackets", "minus_equal"] as const;

export const linuxWheelPagingNote =
  "鼠标滚轮：开启后在 IBus 候选窗口上滚动即翻页，关闭时滚轮不做任何事。Fcitx5 经典界面的滚轮翻页是 Fcitx5 自己的设置，开启或改回关闭后会同步写入，对 Fcitx5 中的所有输入法生效；从未开启过时沿用 Fcitx5 原有设置。";

export function navigationShortcutItems(navigation: NavigationPreferences, harmonyPhone = false) {
  return (harmonyPhone ? harmonyPhoneNavigationOptions : navigationOptions).map(
    ([value, label]) => ({
      value,
      label,
      // `mouse_wheel` 是可选的；未设置即关闭。左右键出厂为开，缺键读作开。
      checked: navigation[value] ?? value === "left_right",
    }),
  );
}

export function wordCharacterKeyOptions(navigation: NavigationPreferences) {
  return wordCharacterChoices.map((value) => ({
    value,
    label: wordCharacterKeys[value],
    disabled: navigation[value],
  }));
}

export function enabledPagingShortcuts(navigation: NavigationPreferences) {
  return pagingKeys.filter(([option]) => navigation[option]);
}
