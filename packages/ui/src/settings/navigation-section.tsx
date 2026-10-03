import { SettingsGroupNote } from "./settings-group-note";
import { Checks } from "../core/platform-controls";
import * as settings from "./settings-style";
import type { NavigationPreferences, WordCharacterPreferences } from "./word-character-section";

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

const linuxWheelPagingNote =
  "鼠标滚轮：开启后在 IBus 候选窗口上滚动即翻页，关闭时滚轮不做任何事。Fcitx5 经典界面的滚轮翻页是 Fcitx5 自己的设置，开启或改回关闭后会同步写入，对 Fcitx5 中的所有输入法生效；从未开启过时沿用 Fcitx5 原有设置。";

export interface NavigationSectionProps {
  navigation: NavigationPreferences;
  wordCharacter: WordCharacterPreferences;
  linux: boolean;
  onChange: (next: {
    navigation: NavigationPreferences;
    wordCharacter: WordCharacterPreferences;
  }) => void;
}

/** 共用的候选翻页设置及其与以词定字的互斥：输入页「选词与翻页」组里以词定字之后的部分。 */
export function NavigationSection({
  navigation,
  wordCharacter,
  linux,
  onChange,
}: NavigationSectionProps) {
  return (
    <>
      <div className={settings.groupBlock}>
        <Checks
          legend="翻页方式"
          items={navigationOptions.map(([key, label]) => ({
            value: key,
            label,
            // `mouse_wheel` is optional; unset is off, as it always drew.
            checked: navigation[key] ?? false,
          }))}
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
      </div>
      {/* IBus pages on the panel's cursor_up/down and button 4/5 only with the switch on; Fcitx5 classic UI pages by itself, so the host writes the switch into classicui's WheelForPaging once it leaves the default (platforms/linux/README.md). */}
      {linux && <SettingsGroupNote>{linuxWheelPagingNote}</SettingsGroupNote>}
    </>
  );
}
